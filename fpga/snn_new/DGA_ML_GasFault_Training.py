"""Two-stage DGA training: sensor response -> gas scores -> fault class.

Run from the project directory. Each CSV is one sample; each worksheet in an
XLSX workbook is one sample. Workbook sheets stay in the same split group.
"""
from __future__ import annotations
import argparse, json, re, warnings, sys
from datetime import datetime
from pathlib import Path
import joblib
import numpy as np
import pandas as pd
import torch
from sklearn.ensemble import RandomForestClassifier
from sklearn.feature_selection import f_classif
from sklearn.impute import SimpleImputer
from sklearn.base import BaseEstimator, ClassifierMixin
from sklearn.metrics import confusion_matrix, accuracy_score, f1_score
from sklearn.model_selection import StratifiedGroupKFold, LeaveOneGroupOut
from sklearn.multioutput import MultiOutputClassifier
from sklearn.pipeline import make_pipeline
from sklearn.preprocessing import StandardScaler
from sklearn.svm import SVC
from tsfresh import extract_features
from tsfresh.feature_extraction import EfficientFCParameters

GASES = ['C2H2','C2H4','CH4','H2','OTHER']
FAULTS = ['T1','D1','N']
MOS = [f'MOS{i}' for i in range(16)]
# Give the custom torch estimator stable import paths when this file is run as
# a script; the exported joblib bundle is later loaded by predict.py.
sys.modules.setdefault('DGA_ML_GasFault_Training', sys.modules[__name__])

def has_mos_columns(columns):
    return any(
        (m := re.match(r'^\s*MOS\s*(\d+)', str(col), flags=re.IGNORECASE))
        and int(m.group(1)) < 16
        for col in columns
    )

def log(message):
    print(f"[{datetime.now().strftime('%H:%M:%S')}] {message}",flush=True)

class _Spike(torch.autograd.Function):
    """Hard threshold in the forward pass, smooth surrogate derivative."""
    @staticmethod
    def forward(ctx, voltage):
        ctx.save_for_backward(voltage)
        return (voltage > 0).to(voltage.dtype)

    @staticmethod
    def backward(ctx, grad):
        (voltage,) = ctx.saved_tensors
        return grad / (1.0 + voltage.abs()).pow(2)

class _LIFNet(torch.nn.Module):
    def __init__(self, n_features, hidden):
        super().__init__()
        self.input = torch.nn.Linear(n_features, hidden)
        self.readout = torch.nn.Linear(hidden, 1)
        self.hidden = hidden
    def forward(self, value):
        membrane = torch.zeros((value.shape[0], self.hidden), device=value.device)
        spikes_sum = torch.zeros_like(membrane)
        current = torch.relu(self.input(value))
        for _ in range(8):
            membrane = 0.85 * membrane + current - 1.0
            spikes = _Spike.apply(membrane)
            membrane = membrane * (1.0 - spikes)
            spikes_sum = spikes_sum + spikes
        return self.readout(spikes_sum / 8.0)

class SNNBinaryClassifier(BaseEstimator, ClassifierMixin):
    """Small leaky integrate-and-fire binary classifier for one-vs-rest gas labels."""
    def __init__(self, epochs=35, hidden=24, lr=0.005, weight_decay=0.001, random_state=42):
        self.epochs=epochs; self.hidden=hidden; self.lr=lr; self.weight_decay=weight_decay; self.random_state=random_state
    def fit(self, X, y):
        import torch
        torch.manual_seed(self.random_state)
        x=torch.as_tensor(np.asarray(X,dtype=np.float32)); yy=torch.as_tensor(np.asarray(y,dtype=np.float32).reshape(-1,1))
        observed=np.unique(np.asarray(y))
        self.classes_=np.array([0,1],dtype=int)
        if len(observed) < 2:
            self.constant_=int(observed[0]); self.net_=None; return self
        self.net_=_LIFNet(x.shape[1],self.hidden)
        opt=torch.optim.Adam(self.net_.parameters(),lr=self.lr,weight_decay=self.weight_decay)
        # One-vs-rest labels can be imbalanced. Weight positive examples so
        # minority gas classes are not learned as an almost-constant negative.
        positive=float(yy.sum().item()); negative=float(len(yy)-positive)
        pos_weight=min(20.0,negative/max(positive,1.0))
        loss_fn=torch.nn.BCEWithLogitsLoss(pos_weight=torch.tensor([pos_weight],dtype=torch.float32))
        self.net_.train()
        for _ in range(self.epochs):
            opt.zero_grad(); loss=loss_fn(self.net_(x),yy); loss.backward(); opt.step()
        self.net_.eval(); return self
    def predict_proba(self, X):
        if self.net_ is None:
            p=np.ones(len(X)) if self.constant_==1 else np.zeros(len(X))
        else:
            import torch
            with torch.no_grad(): p=torch.sigmoid(self.net_(torch.as_tensor(np.asarray(X,dtype=np.float32)))).numpy().ravel()
        return np.column_stack([1-p,p])
    def predict(self, X):
        return (self.predict_proba(X)[:,1]>=0.5).astype(int)

_Spike.__module__='DGA_ML_GasFault_Training'
_LIFNet.__module__='DGA_ML_GasFault_Training'
SNNBinaryClassifier.__module__='DGA_ML_GasFault_Training'

def recordings(folder, classes, excel=False):
    out=[]
    for label in classes:
        d=folder/label
        if not d.exists(): continue
        for p in sorted(d.iterdir()):
            if p.is_file() and p.suffix.lower()=='.csv': out.append((p,None,label))
            elif excel and p.suffix.lower() in ('.xlsx','.xls'):
                try:
                    book=pd.ExcelFile(p)
                    for sheet in book.sheet_names:
                        # Some workbooks begin with a run summary sheet. It is
                        # metadata, not a sensor recording, and must not become
                        # a training example.
                        try:
                            header=pd.read_excel(p,sheet_name=sheet,nrows=0)
                            if has_mos_columns(header.columns):
                                out.append((p,sheet,label))
                            else:
                                warnings.warn(f'Skip non-sensor sheet {p.name} / {sheet}')
                        except Exception as e:
                            warnings.warn(f'Skip unreadable sheet {p.name} / {sheet}: {e}')
                except Exception as e: warnings.warn(f'Skip {p}: {e}')
    return out

def read_record(item):
    p,s,_=item
    return pd.read_excel(p,sheet_name=s) if s is not None else pd.read_csv(p)

def session_group(path):
    """Group cycle CSVs from the same recorded session to prevent leakage."""
    p=Path(path)
    stem=re.sub(r'_cycle_\d+(?=_|$)','',p.stem,flags=re.IGNORECASE)
    return str(p.parent / stem)

def extract_tsfresh(items):
    rows=[]
    for i,item in enumerate(items):
        df=read_record(item)
        # Accept both bare MOS0...MOS15 headers and real dataset headers such
        # as mos0_mV. Normalize to canonical channel names for TSFRESH.
        rename={}
        for col in df.columns:
            match=re.match(r'^\s*MOS\s*(\d+)',str(col),flags=re.IGNORECASE)
            if match and int(match.group(1)) < 16:
                rename[col]=f"MOS{int(match.group(1))}"
        normalized=df.rename(columns=rename)
        available=[c for c in MOS if c in normalized.columns]
        if not available:
            raise ValueError(f'No MOS0-MOS15 columns in {item[0]} (headers: {list(df.columns)[:12]})')
        # In case a file contains duplicate aliases for a sensor, retain the
        # first canonical column instead of passing duplicate names downstream.
        sensor=normalized.loc[:,available].copy()
        sensor=sensor.loc[:,~sensor.columns.duplicated()]
        long=sensor.apply(pd.to_numeric,errors='coerce').reset_index().rename(columns={'index':'time'})
        long['id']=i
        rows.append(long.melt(id_vars=['id','time'],var_name='channel',value_name='value').dropna())
    ts=pd.concat(rows,ignore_index=True)
    return extract_features(ts,column_id='id',column_sort='time',column_kind='channel',column_value='value',default_fc_parameters=EfficientFCParameters(),n_jobs=0,disable_progressbar=False)

def matrix(x,names,medians=None):
    x=x.reindex(columns=names).replace([np.inf,-np.inf],np.nan)
    if medians is None: medians=x.median().fillna(0)
    return x.fillna(medians).fillna(0).to_numpy(float),medians

def select_features(x,y,limit=100,context='feature selection',n_independent=None):
    # Limit model capacity relative to the number of independent training
    # recordings. This is especially important inside small grouped CV folds.
    independent_count=int(n_independent if n_independent is not None else x.shape[0])
    limit=min(limit,max(5,independent_count//3))
    log(f'{context}: mulai uji relevansi pada {x.shape[0]} sampel/{independent_count} sesi independen, {x.shape[1]} fitur; target fitur maks={limit}')
    z=x.replace([np.inf,-np.inf],np.nan); med=z.median().fillna(0); z=z.fillna(med).fillna(0)
    with warnings.catch_warnings():
        warnings.simplefilter('ignore'); f,p=f_classif(z.to_numpy(),y)
    p=np.nan_to_num(p,nan=1.0); candidates=np.flatnonzero(p<.05)
    if not len(candidates): candidates=np.argsort(np.nan_to_num(f,nan=0))[-min(500,z.shape[1]):]
    log(f'{context}: {len(candidates)} kandidat relevan; mulai ranking Random Forest')
    rf=RandomForestClassifier(n_estimators=160,max_depth=8,min_samples_leaf=2,max_features='sqrt',class_weight='balanced',random_state=42,n_jobs=-1).fit(z.iloc[:,candidates],y)
    order=candidates[np.argsort(rf.feature_importances_)[::-1]]
    log(f'{context}: ranking selesai; mulai pruning korelasi')
    # Greedy correlation pruning avoids allocating a potentially huge NxN
    # correlation matrix when TSFRESH emits thousands of candidate features.
    ordered=z.iloc[:,order]; chosen=[]
    for j in range(len(order)):
        if chosen:
            corr_to_selected=ordered.iloc[:,chosen].corrwith(ordered.iloc[:,j]).abs()
            if (corr_to_selected>=.98).any():
                continue
        chosen.append(j)
        if len(chosen)>=limit: break
    names=list(z.columns[order[chosen]])
    log(f'{context}: seleksi selesai, {len(names)} fitur dipilih')
    return names,med[names]

def gas_models():
    return {
      'SNN':make_pipeline(SimpleImputer(strategy='median'),StandardScaler(),MultiOutputClassifier(SNNBinaryClassifier(),n_jobs=1)),
      'RandomForest':MultiOutputClassifier(RandomForestClassifier(n_estimators=240,max_depth=8,min_samples_leaf=2,max_features='sqrt',class_weight='balanced',random_state=42,n_jobs=-1),n_jobs=-1),
      'SVM':make_pipeline(SimpleImputer(strategy='median'),StandardScaler(),MultiOutputClassifier(SVC(kernel='rbf',probability=True,class_weight='balanced',random_state=42),n_jobs=-1))}

def gas_confidences(model,x):
    probs=model.predict_proba(x)
    return np.column_stack([p[:,list(estimator.classes_).index(1)] if 1 in estimator.classes_ else np.zeros(len(x)) for p,estimator in zip(probs,model[-1].estimators_ if hasattr(model,'steps') else model.estimators_)])

def save_cm(out,name,y,pred,labels):
    pd.DataFrame(confusion_matrix(y,pred,labels=labels),index=labels,columns=labels).to_csv(out/f'{name}_confusion.csv')

def main():
    ap=argparse.ArgumentParser(); ap.add_argument('--data-dir',default='data'); ap.add_argument('--output-dir',default='models_gas_fault'); ap.add_argument('--seed',type=int,default=42); args=ap.parse_args()
    try:
        import reportlab  # noqa: F401
    except ImportError as exc:
        raise RuntimeError('Laporan PDF memerlukan reportlab. Pasang lebih dulu dengan: pip install reportlab') from exc
    root=Path(args.data_dir); out=Path(args.output_dir); out.mkdir(parents=True,exist_ok=True)
    gas=recordings(root/'Dissolved_Gas',GASES,excel=True)
    fault=recordings(root/'Class',FAULTS)
    unseen=recordings(root/'Class',['UNSEEN'])
    if not gas or not fault: raise ValueError('Need data/Dissolved_Gas/{C2H2,C2H4,CH4,H2,OTHER} and data/Class/{T1,D1,N}.')
    log('Data gas per kelas: '+', '.join(f'{g}={sum(r[2]==g for r in gas)}' for g in GASES))
    log('Data fault per kelas: '+', '.join(f'{c}={sum(r[2]==c for r in fault)}' for c in FAULTS))
    log(f'Folder output: {out.resolve()}')
    log(f'STAGE 1/8: ekstraksi TSFRESH gas={len(gas)} (CSV/XLSX sheets), fault={len(fault)}, unseen={len(unseen)}')
    gx=extract_tsfresh(gas); gy=np.array([r[2] for r in gas]); groups=np.array([session_group(r[0]) for r in gas])
    log(f'STAGE 1/8 selesai: fitur gas berukuran {gx.shape}')
    log('Sesi independen gas per kelas: '+', '.join(f'{g}={len(np.unique(groups[gy==g]))}' for g in GASES))
    # Stratified grouped holdout; sheets from one workbook cannot leak across partitions.
    splitter=StratifiedGroupKFold(n_splits=5,shuffle=True,random_state=args.seed)
    valid_splits=[]
    for a,b in splitter.split(gx,gy,groups):
        if set(np.unique(gy[a]))==set(GASES) and set(np.unique(gy[b]))==set(GASES):
            valid_splits.append((a,b))
    if not valid_splits:
        raise ValueError('Tidak ada split holdout bergroup yang memuat semua kelas gas di train dan holdout. Periksa jumlah sesi per kelas; evaluasi tidak boleh melatih model tanpa salah satu gas.')
    tr,te=valid_splits[0]
    log(f'STAGE 2/8: holdout gas siap; train={len(tr)}, holdout={len(te)}')
    names,med=select_features(gx.iloc[tr],gy[tr],context='Holdout gas',n_independent=len(np.unique(groups[tr]))); xtr,_=matrix(gx.iloc[tr],names,med); xte,_=matrix(gx.iloc[te],names,med)
    ytr=gy[tr]; yte=gy[te]; ytr_bin=np.column_stack([ytr==g for g in GASES]).astype(int); yte_bin=np.column_stack([yte==g for g in GASES]).astype(int)
    metrics=[]
    for name,model in gas_models().items():
        log(f'STAGE 3/8: training model gas {name} pada holdout split')
        model.fit(xtr,ytr_bin); scores=gas_confidences(model,xte); pred=np.array(GASES)[scores.argmax(axis=1)]
        save_cm(out,f'gas_{name.lower()}_holdout',yte,pred,GASES)
        metrics.append({'model':name,'holdout_accuracy':accuracy_score(yte,pred),'holdout_macro_f1':f1_score(yte,pred,labels=GASES,average='macro',zero_division=0)})
        log(f'Model {name} selesai: accuracy={metrics[-1]["holdout_accuracy"]:.3f}, macro-F1={metrics[-1]["holdout_macro_f1"]:.3f}')
    pd.DataFrame(metrics).to_csv(out/'gas_model_comparison.csv',index=False)
    log(f'Hasil holdout gas (pelaporan saja, tidak untuk memilih model): {metrics}')
    # K-fold, LOMO and LOCO evaluations on the gas development partition.
    cv_rows=[]; cv_metric_rows=[]
    kfold_splits=list(splitter.split(gx.iloc[tr],ytr,groups[tr]))
    log(f'STAGE 4/8: K-fold gas mulai, {len(kfold_splits)} fold; model dibandingkan=SNN, SVM, RandomForest')
    for fold,(a,b) in enumerate(kfold_splits,1):
        log(f'K-fold gas {fold}/{len(kfold_splits)}: mulai ({len(a)} train, {len(b)} validasi)')
        if set(np.unique(ytr[a]))!=set(GASES) or set(np.unique(ytr[b]))!=set(GASES):
            log(f'K-fold gas {fold}: dilewati karena salah satu subset tidak memuat semua kelas')
            continue
        nn,mm=select_features(gx.iloc[tr].iloc[a],ytr[a],context=f'K-fold gas {fold}',n_independent=len(np.unique(groups[tr][a]))); xa,_=matrix(gx.iloc[tr].iloc[a],nn,mm); xb,_=matrix(gx.iloc[tr].iloc[b],nn,mm)
        yy=np.column_stack([ytr[a]==g for g in GASES]).astype(int)
        for model_name,model in gas_models().items():
            log(f'K-fold gas {fold}/{len(kfold_splits)}: training {model_name}')
            model.fit(xa,yy); fold_scores=gas_confidences(model,xb); pp=np.array(GASES)[fold_scores.argmax(axis=1)]
            cv_metric_rows.append({'model':model_name,'fold':fold,'accuracy':accuracy_score(ytr[b],pp),'macro_f1':f1_score(ytr[b],pp,labels=GASES,average='macro',zero_division=0),'support':len(b)})
            cv_rows.extend({'model':model_name,'fold':fold,'actual':u,'predicted':v,**{f'confidence_{g}':float(s*100) for g,s in zip(GASES,row)}} for u,v,row in zip(ytr[b],pp,fold_scores))
            log(f'K-fold gas {fold}/{len(kfold_splits)} {model_name} selesai: macro-F1={cv_metric_rows[-1]["macro_f1"]:.3f}')
    pd.DataFrame(cv_rows).to_csv(out/'gas_kfold_predictions.csv',index=False)
    cv_detail=pd.DataFrame(cv_metric_rows)
    cv_summary=cv_detail.groupby('model').agg(accuracy_mean=('accuracy','mean'),accuracy_std=('accuracy','std'),macro_f1_mean=('macro_f1','mean'),macro_f1_std=('macro_f1','std'),folds=('fold','count')).reset_index().fillna(0)
    cv_summary.to_csv(out/'gas_kfold_metrics.csv',index=False)
    log(f'STAGE 4/8 selesai: K-fold membandingkan SNN, SVM, dan RF; ringkasan={cv_summary.to_dict(orient="records")}')
    logo=LeaveOneGroupOut(); lomo=[]; lomo_splits=list(logo.split(gx.iloc[tr],ytr,groups[tr]))
    log(f'STAGE 5/8: LOMO gas mulai, {len(lomo_splits)} grup')
    for fold,(a,b) in enumerate(lomo_splits,1):
        if fold==1 or fold%5==0 or fold==len(lomo_splits): log(f'LOMO gas {fold}/{len(lomo_splits)}: mulai')
        if set(np.unique(ytr[a]))!=set(GASES): continue
        nn,mm=select_features(gx.iloc[tr].iloc[a],ytr[a],context=f'LOMO gas SNN {fold}',n_independent=len(np.unique(groups[tr][a]))); xa,_=matrix(gx.iloc[tr].iloc[a],nn,mm); xb,_=matrix(gx.iloc[tr].iloc[b],nn,mm)
        yy=np.column_stack([ytr[a]==g for g in GASES]).astype(int); model=gas_models()['SNN'].fit(xa,yy); fold_scores=gas_confidences(model,xb); pp=np.array(GASES)[fold_scores.argmax(axis=1)]
        lomo.extend({'model':'SNN','fold':fold,'actual':u,'predicted':v,**{f'confidence_{g}':float(s*100) for g,s in zip(GASES,row)}} for u,v,row in zip(ytr[b],pp,fold_scores))
        log(f'LOMO gas {fold}/{len(lomo_splits)} selesai')
    pd.DataFrame(lomo).to_csv(out/'gas_lomo_predictions.csv',index=False)
    log(f'STAGE 5/8 selesai: LOMO gas; prediksi tersimpan ({len(lomo)} baris)')
    # Leave-one-MOS-channel-out ablation, using final holdout.
    loco=[]
    log('STAGE 6/8: LOCO gas mulai, menguji penghilangan tiap kanal MOS')
    for ch in range(16):
        log(f'LOCO gas kanal MOS{ch}: training/evaluasi')
        reduced=[n for n in names if not n.startswith(f'MOS{ch}__')]
        if not reduced: continue
        xa,_=matrix(gx.iloc[tr],reduced); xb,_=matrix(gx.iloc[te],reduced); yy=ytr_bin
        model=gas_models()['SNN'].fit(xa,yy); pp=np.array(GASES)[gas_confidences(model,xb).argmax(axis=1)]
        loco.append({'left_out_channel':f'MOS{ch}','accuracy':accuracy_score(yte,pp),'macro_f1':f1_score(yte,pp,labels=GASES,average='macro',zero_division=0)})
        log(f'LOCO MOS{ch} selesai: accuracy={loco[-1]["accuracy"]:.3f}, macro-F1={loco[-1]["macro_f1"]:.3f}')
    pd.DataFrame(loco).to_csv(out/'gas_loco_metrics.csv',index=False)
    log('STAGE 6/8 selesai: LOCO gas')
    # Final gas model and scores for fault examples.
    log('STAGE 7/8: seleksi fitur final dan refit SNN gas memakai seluruh data berlabel')
    final_names,final_medians=select_features(gx,gy,context='Refit gas final',n_independent=len(np.unique(groups))); xall,_=matrix(gx,final_names,final_medians); yall=np.column_stack([gy==g for g in GASES]).astype(int)
    # Final deployment artifact is always the evaluated SNN, regardless of
    # which candidate has the highest holdout macro-F1.
    log(f'Refit SNN final mulai: {len(gy)} sampel, {len(final_names)} fitur')
    final_gas=gas_models()['SNN'].fit(xall,yall); log('Refit SNN final selesai; menulis gas_model.joblib')
    joblib.dump({'model':final_gas,'feature_names':final_names,'imputation_values':final_medians,'gas_classes':GASES,'selected_model':'SNN'},out/'gas_model.joblib')
    log('STAGE 8/8: ekstraksi TSFRESH untuk data fault dan pembuatan confidence gas')
    fx=extract_tsfresh(fault); log(f'TSFRESH fault selesai: fitur berukuran {fx.shape}')
    fscore=gas_confidences(final_gas,matrix(fx,final_names,final_medians)[0]); fy=np.array([r[2] for r in fault]); fg=np.array([session_group(r[0]) for r in fault])
    # Session holdout: group by source file; choose groups while retaining each class in train.
    log(f'Confidence gas fault selesai; ditemukan {len(np.unique(fg))} grup sesi fault')
    test=[]
    for label in FAULTS:
        ix=np.flatnonzero(fy==label); unique=list(dict.fromkeys(fg[ix]))
        if len(unique)>1: test.extend(np.flatnonzero((fy==label)&(fg==unique[-1])).tolist())
    test=np.array(test,dtype=int); train=np.array([i for i in range(len(fault)) if i not in set(test)],dtype=int)
    log(f'Evaluasi fault holdout per sesi: train={len(train)}, holdout={len(test)}')
    log(f'Evaluasi holdout fault per sesi: train={len(train)}, holdout={len(test)}')
    if len(test) and len(np.unique(fy[train]))==3:
        fm=RandomForestClassifier(n_estimators=300,max_depth=3,min_samples_leaf=2,max_features='sqrt',class_weight='balanced',random_state=args.seed).fit(fscore[train],fy[train]); fp=fm.predict(fscore[test]); save_cm(out,'fault_holdout',fy[test],fp,FAULTS)
        pd.DataFrame({'actual':fy[test],'predicted':fp,'session':[fg[i] for i in test]}).to_csv(out/'fault_holdout_predictions.csv',index=False)
        log(f'Holdout fault selesai: accuracy={accuracy_score(fy[test],fp):.3f}, macro-F1={f1_score(fy[test],fp,labels=FAULTS,average="macro",zero_division=0):.3f}')
    else:
        log('PERINGATAN: holdout fault tidak tersedia karena sesi kurang atau kelas train tidak lengkap')
    # LOMO fault session evaluation.
    fl=[]; fault_lomo_splits=list(LeaveOneGroupOut().split(fscore,fy,fg))
    log(f'LOMO fault mulai, {len(fault_lomo_splits)} sesi')
    for fold,(a,b) in enumerate(fault_lomo_splits,1):
        if fold==1 or fold%5==0 or fold==len(fault_lomo_splits): log(f'LOMO fault {fold}/{len(fault_lomo_splits)}: mulai')
        if set(np.unique(fy[a]))!=set(FAULTS): continue
        fm=RandomForestClassifier(n_estimators=200,max_depth=3,min_samples_leaf=2,max_features='sqrt',class_weight='balanced',random_state=args.seed).fit(fscore[a],fy[a]); pp=fm.predict(fscore[b])
        fl.extend({'fold':fold,'actual':u,'predicted':v} for u,v in zip(fy[b],pp))
        log(f'LOMO fault sesi {fold}/{len(fault_lomo_splits)} selesai')
    pd.DataFrame(fl).to_csv(out/'fault_lomo_predictions.csv',index=False)
    log('Refit model fault final dengan seluruh data fault')
    log('Refit model fault final dimulai dengan seluruh data fault')
    final_fault=RandomForestClassifier(n_estimators=300,max_depth=3,min_samples_leaf=2,max_features='sqrt',class_weight='balanced',random_state=args.seed).fit(fscore,fy); joblib.dump({'model':final_fault,'fault_classes':FAULTS,'gas_classes':GASES},out/'fault_model.joblib')
    log('Refit model fault selesai; model tersimpan di fault_model.joblib')
    pd.DataFrame([{'source':str(r[0]),'fault_label':r[2],**{f'confidence_{g}':float(s*100) for g,s in zip(GASES,row)}} for r,row in zip(fault,fscore)]).to_csv(out/'class_gas_confidences.csv',index=False)
    if unseen:
        log(f'Prediksi unseen mulai untuk {len(unseen)} file')
        ux=extract_tsfresh(unseen); log(f'Fitur unseen selesai diekstrak: {ux.shape}')
        us=gas_confidences(final_gas,matrix(ux,final_names,final_medians)[0]); up=final_fault.predict(us)
        pd.DataFrame([{'file':str(r[0]),'fault_prediction':cl,**{f'confidence_{g}':round(float(v*100),2) for g,v in zip(GASES,row)}} for r,row,cl in zip(unseen,us,up)]).to_csv(out/'unseen_predictions.csv',index=False)
        log('Prediksi unseen tersimpan di unseen_predictions.csv')
    meta={'gas_training_files':len(gas),'gas_independent_sessions':int(len(np.unique(groups))),'fault_training_files':len(fault),'fault_independent_sessions':int(len(np.unique(fg))),'unseen_files':len(unseen),'feature_count':len(final_names),'gas_models_evaluated':['SNN','SVM','RandomForest'],'gas_model_exported':'SNN','gas_cv_models_compared':['SNN','SVM','RandomForest'],'fault_model_exported':'RandomForest','regularization':{'SNN_hidden':24,'SNN_weight_decay':0.001,'gas_max_features_per_fold':'one-third of independent training sessions','RF_gas_max_depth':8,'RF_fault_max_depth':3,'RF_min_samples_leaf':2},'synthetic_augmentation':False,'evaluations':['session-grouped 80/20 holdout for reporting','session-grouped 5-fold CV for SNN/SVM/RandomForest','session-grouped LOMO for SNN','LOCO MOS channel ablation for SNN'],'fault_evaluations':['session holdout','LOMO session'],'gas_classes':GASES,'fault_classes':FAULTS}
    (out/'pipeline_metadata.json').write_text(json.dumps(meta,indent=2),encoding='utf-8')
    log('Menyusun laporan PDF dari confusion matrix, metrik CV, confidence dan prediksi unseen')
    try:
        from dga_report import build_report
        pdf_path=build_report(out)
        log(f'Laporan PDF selesai: {pdf_path.name}')
    except Exception as e:
        raise RuntimeError(f'Pembuatan laporan PDF gagal: {e}') from e
    # Detailed CSVs are intermediate report inputs. Preserve only the unseen
    # predictions as a practical, machine-readable result alongside the models.
    removed=[]
    for artifact in out.glob('*.csv'):
        if artifact.name != 'unseen_predictions.csv':
            artifact.unlink()
            removed.append(artifact.name)
    old_html=out/'Laporan_Training_DGA.html'
    if old_html.exists(): old_html.unlink()
    log(f'CSV evaluasi perantara dibersihkan: {len(removed)} file')
    log('PIPELINE SELESAI. File akhir: '+', '.join(p.name for p in sorted(out.iterdir()) if p.is_file()))
    log(f'Lokasi output: {out.resolve()}')

if __name__=='__main__': main()
