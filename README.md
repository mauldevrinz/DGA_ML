# DGA Intelligent Analysis

Aplikasi untuk mengakuisisi respons 16 sensor MOS, menjalankan inferensi gas dan klasifikasi kondisi minyak transformator, serta meninjau hasilnya melalui dashboard. Sistem dirancang untuk **Kria KV260 dengan PetaLinux** dan Teensy 4.1.

Antarmuka dibangun dengan React dan Vite. Python menangani komunikasi USB, WebSocket, dan ekstraksi fitur time-series. Binary Rust menjalankan pipeline SNN dan klasifikasi fault, termasuk akses ke accelerator FPGA.

## Isi

- [Cara kerja](#cara-kerja)
- [Perangkat dan kebutuhan](#perangkat-dan-kebutuhan)
- [Menjalankan pada Kria KV260](#menjalankan-pada-kria-kv260)
- [Menggunakan aplikasi](#menggunakan-aplikasi)
- [Model dan interpretasi hasil](#model-dan-interpretasi-hasil)
- [Penyimpanan dan pemecahan masalah](#penyimpanan-dan-pemecahan-masalah)
- [Struktur proyek](#struktur-proyek)

## Cara kerja

```text
16 kanal MOS pada Teensy 4.1
        │ USB bulk IN
        ▼
serial_ws.py ── WebSocket :8080 ── React dashboard
        │                              │
        │ rekaman satu cycle            ├─ akuisisi dan kontrol system plan
        │ atau file CSV                 └─ menampilkan hasil dan history
        ▼
Python tsfresh: 37 fitur time-series
        │ JSON melalui stdin/stdout
        ▼
Rust snn_infer ── SNN di FPGA + klasifikasi fault Random Forest
        │
        └── prediksi gas, fault, latency, dan metrik daya (jika tersedia)
```

Model menggunakan 16 nilai sensor MOS per sampel. Backend mengirim rekaman sebagai matriks `N × 16` ke ekstraktor Python, yang menghitung 37 fitur sesuai metadata training. Binary Rust menggunakan fitur tersebut untuk inferensi gas SNN dan klasifikasi fault.

## Perangkat dan kebutuhan

- Kria KV260 yang menjalankan PetaLinux.
- Teensy 4.1 terhubung melalui USB dan firmware yang mengirim frame sensor yang didukung backend.
- Bitstream dan metadata hardware accelerator: `fpga/snn_new/snn.bit` dan `fpga/snn_new/snn.hwh`.
- Python 3, pip, Node.js/npm, Rust/Cargo, serta utilitas FPGA PetaLinux `xmutil`, `dtc`, dan akses `sudo`.
- Dependencies Python di [`requirements.txt`](requirements.txt) serta dependencies frontend/Rust dalam manifest proyek.

> `run.sh` memeriksa InfluxDB, tetapi InfluxDB tidak wajib untuk inferensi: SQLite tetap menyimpan data sensor bila InfluxDB tidak tersedia.

## Menjalankan pada Kria KV260

Clone/copy proyek ke Kria, masuk ke direktori proyek, lalu siapkan dependencies:

```bash
python3 -m pip install -r requirements.txt
npm install
```

Jalankan aplikasi:

```bash
./run.sh
```

Script ini meminta akses sudo, memuat accelerator SNN jika overlay belum aktif, membangun binary persistence dan inferensi Rust dalam mode release, memulai `serial_ws.py`, lalu menjalankan Vite. Buka alamat yang dicetak Vite dari browser pada Kria atau perangkat di jaringan yang sama. Backend WebSocket mendengarkan pada port `8080`.

Loader FPGA dapat dijalankan terpisah bila diperlukan:

```bash
sudo ./fpga/load_snn.sh
```

Script loader mengonversi `.bit` ke `.bin` bila perlu, menyiapkan overlay, dan memuat aplikasi `snn_accel` menggunakan `xmutil`. Backend memerlukan izin yang sesuai untuk USB dan akses perangkat FPGA; `run.sh` menjalankan backend menggunakan sudo.

Untuk pengembangan UI tanpa memulai seluruh stack perangkat:

```bash
npm run dev
```

Mode ini hanya menjalankan frontend. Fitur yang membutuhkan sensor, WebSocket, atau FPGA memerlukan backend berjalan secara terpisah.

### Opsi konfigurasi backend

Backend membaca opsi berikut melalui environment variable, saat dijalankan langsung:

| Variable | Default | Kegunaan |
|---|---|---|
| `SNN_INFER_BIN` | `fpga/snn_infer/target/release/snn_infer` | Lokasi binary inferensi Rust |
| `SNN_MODELS_DIR` | `fpga/snn_infer/models` (dengan fallback di binary) | Lokasi model SNN dan Random Forest hasil export untuk Rust |

Pastikan model dan metadata fitur berasal dari training/export yang sama. Model Python `.joblib` di `fpga/snn_new/` adalah artefak training; binary runtime Rust membaca model hasil export di `fpga/snn_infer/models/`.

## Menggunakan aplikasi

### Akuisisi dan system plan

1. Sambungkan Teensy, lalu hubungkan stream di halaman **Acquisition**.
2. Atur durasi fase dan jumlah siklus pada system plan.
3. Tekan **Start System**. Backend mengirim perintah fase ke Teensy dan menjadwalkan fase seperti `IDLE`, `INJECT`, dan `PURGE` untuk setiap cycle.
4. Data sensor diterima dan direkam per cycle. Hentikan sistem dengan kontrol **Stop System** jika diperlukan.

System plan dijalankan backend, bukan bergantung pada timer browser. Setiap rekaman cycle yang dikirim untuk inferensi menggunakan sampel yang diterima pada cycle tersebut. Backend memerlukan minimal 10 sampel per cycle dan membatasi rekaman ke 3.600 sampel.

### Inferensi FPGA otomatis per cycle

Di halaman **Classification**, tombol **Start FPGA Inference** berfungsi sebagai sakelar:

- Saat aktif, inferensi otomatis dijalankan **sekali setelah setiap cycle dalam system plan selesai**.
- Saat nonaktif, system plan tetap dapat berjalan dan merekam data, tetapi inferensi otomatis per cycle tidak dijalankan.
- Jika diaktifkan ketika system plan sedang berjalan, inferensi mulai dari cycle berikutnya.
- Hasil sukses masuk ke dashboard dan **Inference History**, dengan nomor cycle, jumlah sampel, gas terprediksi, fault, dan latency.
- Cycle yang gagal atau tidak memiliki data mencukupi mengirim error dan tidak dihitung sebagai prediksi sukses.

> Tombol ini mengaktifkan inferensi otomatis setelah satu cycle selesai, bukan inferensi snapshot berulang setiap detik.

### Prediksi data historis / CSV

Pada **Classification**, pilih file CSV historis untuk menjalankan prediksi pada rekaman penuh. CSV harus memiliki minimal 10 baris dan nilai untuk 16 kanal MOS. Backend menerima nama kanal seperti `mos0`, `mos0_mV`, atau `MOS0`. Hasil ditampilkan pada kartu hasil analisis. Batas ukuran yang diterima backend adalah 3.600 baris data.

Ekspor data akuisisi dari dashboard menyimpan arsip ZIP berisi CSV terpisah untuk setiap cycle, termasuk nilai sensor MOS dan kolom tambahan yang direkam UI. Untuk inferensi, yang digunakan adalah 16 kanal MOS.

## Model dan interpretasi hasil

- **Ekstraksi fitur:** Python dengan tsfresh, 37 fitur time-series dan metadata fitur hasil training.
- **Klasifikasi gas:** SNN; label keluaran `C2H2`, `C2H4`, `CH4`, `H2`, dan `OTHER` beserta confidence.
- **Klasifikasi fault:** Random Forest hasil export; label `T1`, `D1`, dan `N`.
- **Eksekusi SNN:** binary Rust `snn_infer` meminta eksekusi accelerator FPGA untuk pipeline prediksi. Periksa field `mode` pada hasil untuk membedakan `hw` (hardware) dari mode software/fallback; latency fitur dan inferensi ditampilkan terpisah.
- **Monitoring daya Kria:** ditampilkan bila backend berhasil membaca sensor daya. Jika sensor tidak terdeteksi atau belum ada sampel, metrik daya dapat tampil sebagai `N/A`.

> Prediksi model adalah hasil klasifikasi berdasarkan dataset training, bukan pengukuran langsung konsentrasi gas terlarut atau pengganti diagnosis laboratorium/standar. Dominant gas adalah kelas dengan confidence tertinggi; kelas tersebut sendiri tidak menentukan fault. Jika hasil live tidak sesuai ekspektasi, periksa keterwakilan data per fault, kondisi sensor, dan kesesuaian preprocessing/metadata training.

Metadata model yang disertakan mencatat 112 file training gas dan 43 file training fault; fault berasal dari 7 sesi independen. Jumlah file tidak selalu sama dengan jumlah sampel fisik yang benar-benar independen. Validasi sebaiknya memperhitungkan identitas sampel/sesi untuk menghindari kebocoran data.

## Penyimpanan dan pemecahan masalah

- Backend WebSocket: `ws://<alamat-Kria>:8080`.
- Data stream disimpan melalui service persistence Rust ke SQLite (`sensor_stream.db` di root proyek). Database lokal tidak dilacak Git.
- InfluxDB, jika tersedia di `127.0.0.1:8086`, digunakan sebagai penyimpanan tambahan. Kegagalannya tidak menghentikan penyimpanan SQLite.
- Log backend: `logs/serial_ws.log`.

| Gejala | Yang perlu diperiksa |
|---|---|
| UI tidak tersambung | Pastikan `serial_ws.py` hidup, port 8080 dapat dijangkau, dan browser memakai host Kria yang benar. |
| Teensy tidak mengirim data | Periksa koneksi USB, firmware/format frame, permission USB, serta log backend. |
| Tidak ada hasil inferensi cycle | Pastikan data stream masuk, system plan menyelesaikan cycle, cycle memiliki minimal 10 sampel, dan backend tidak melaporkan overflow atau kegagalan stream. |
| Binary inferensi/model tidak ditemukan | Bangun binary release melalui `run.sh` atau Cargo dan pastikan model tersedia di `fpga/snn_infer/models/`. |
| Accelerator gagal dimuat | Periksa `xmutil`, status overlay/FPGA, file bitstream/overlay, serta jalankan loader dengan sudo. |
| Power menunjukkan N/A | Periksa apakah driver/sensor daya Kria terdeteksi dan backend memperoleh sampel. Prediksi tetap dapat berjalan tanpa telemetri daya. |

## Struktur proyek

```text
src/                         React UI dan halaman aplikasi
src/components/              Acquisition dan Classification dashboard
serial_ws.py                 USB/Teensy, WebSocket, orchestrasi siklus, inferensi
fpga/snn_new/                Bitstream, ekstraksi fitur Python, artefak training
fpga/snn_infer/              Binary Rust SNN/Random Forest dan model runtime
fpga/load_snn.sh             Memuat accelerator Kria KV260
persistence_rs/              Service persistence data sensor
src-tauri/                   Backend/komponen Rust aplikasi desktop
TEENSYCODE/                  Kode Teensy
data/                        Data training lokal
logs/                        Log runtime (dibuat saat aplikasi berjalan)
```

## Perintah pengembangan

```bash
npm run dev       # Jalankan frontend Vite
npm run build     # Build frontend
npm run lint      # Periksa lint frontend
```

Perintah tersebut tidak memuat FPGA atau memulai backend sensor; gunakan `./run.sh` pada perangkat target untuk stack perangkat keras.
