# **Thermal Modelling and Numerical Simulation of an Electronic-Nose Sensor Chamber with 14 MQ/TGS Gas Sensors and Peltier Cooling for Fuzzy-PID Temperature Control** 

(Revised edition, with verified literature support) 

Ahmad Radhy 

Department of Instrumentation Engineering 

Institut Teknologi Sepuluh Nopember, Surabaya, Indonesia 

September 12, 2026 

##### **Abstract** 

Metal-oxide gas sensors of the MQ and TGS families respond to the temperature of the gas around them as well as to its composition. When such sensors are used as an Electronic Nose (E-Nose) for Dissolved Gas Analysis (DGA) of transformer insulating oil, an uncontrolled chamber temperature therefore adds a drift term that is confounded with gas concentration. Controlling the chamber temperature requires a thermal model of the chamber, and the chamber is unusual in that its main heat source is the sensor array it is meant to protect. 

This report derives such a model and uses it to design a temperature controller. The derivation follows a fixed order: chamber geometry, energy balance, coupled differential equations, nonlinear state-space form, numerical implementation in Python, open-loop characterisation, model reduction, a Proportional-Integral-Derivative (PID) baseline, and only then a Fuzzy-PID controller. No reduced transfer function is assumed before the physics produces one. 

Four results follow from the derivation itself. The enclosed air stores about 0.47 J/K, which is two to three orders of magnitude less than the wall and the sensor board. The air is therefore a coupling medium rather than a thermal store, and the effective free-air volume has almost no influence on the thermal dynamics. The linearised radiation coefficient at 303 K is about 5.7 W/(m<sup>2</sup> K) for a high-emissivity internal surface, which is comparable to both natural and fan-driven convection; radiation may be combined with the convective coefficient but should not be neglected without checking the surface finish. The static characteristic from Peltier current to chamber-air temperature is nonlinear, with a local gain that varies by a factor of about three across the usable current range. Finally, a unipolar Peltier drive bounds the reachable setpoints from above by the passive equilibrium temperature and from below by the dew point. 

All numerical results reported here use an explicitly labelled placeholder parameter set. They verify that the model conserves energy, that the zero-input equilibrium is exact, and that the predicted behaviour is physically plausible. They are not predictions of the behaviour of the physical instrument. Every parameter that must be measured before the model becomes predictive is listed with its source and its method of determination. 

**Status of numerical values.** This document distinguishes three classes of numbers. _Derived_ values follow from the stated geometry or from tabulated material properties and are exact given their inputs. _Placeholder_ values, marked<sup>[P]</sup> , are engineering-plausible stand-ins used only so that the code path can be executed and checked; they are not measurements and must be replaced. _Unknown_ values, marked<sup>[U]</sup> , have no numerical value assigned anywhere in this document. No sensor power consumption, Peltier specification, fan airflow or material property has been taken from an unverified source. 

## **Contents** 

- **1 Problem Definition** 

   - 1.1 Instrument context 

**5** 5 

1 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

2 

||1.2|Thermal-modelling objective . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>5|
|---|---|---|---|
||1.3|Control objective . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>5|
||1.4|Method: derivation-first ordering . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>5|
|**2**|**Geo**|**metric Interpretation**|**6**|
||2.1|Given dimensions . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>6|
||2.2|The ambiguity of the 34 mm inner diameter . . . . . . . . . . . . . . . . . .|. . . . . . .<br>6|
|**3**|**Cha**|**mber Volume Calculation**|**7**|
||3.1|Geometric volume<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>7|
||3.2|Occupied volume and effective free-air volume<br>. . . . . . . . . . . . . . . .|. . . . . . .<br>7|
||3.3|How_V_airshould actually be determined<br>. . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>7|
|**4**|**Ther**|**mal-System Boundary**|**8**|
|**5**|**Assu**|**mptions**|**8**|
|**6**|**Heat**|**-Generation Model**|**9**|
||6.1|Electrical power to thermal power<br>. . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>9|
||6.2|Total internal dissipation<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>9|
||6.3|Parameters required . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>10|
|**7**|**Sensi**|**or-Specific Thermal Considerations and Node Aggregation**|**10**|
||7.1|Why the sensor array is thermally awkward<br>. . . . . . . . . . . . . . . . . .|. . . . . . .<br>10|
||7.2|Three candidate representations of the array . . . . . . . . . . . . . . . . . .|. . . . . . .<br>11|
||7.3|What is lost, quantitatively . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>11|
|**8**|**Ther**|**mal-Capacitance Model**|**11**|
||8.1|Numerical evaluation for air<br>. . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>12|
||8.2|Solid capacitances . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>12|
||8.3|Effect on response time . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>12|
|**9**|**Con**|**duction Model**|**12**|
|**10 **|**Con**|**vection Model**|**13**|
||10.1|General form<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>13|
||10.2|Three candidate models . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>13|
|**11 **|**Rad**|**iation Model**|**14**|
||11.1|Order-of-magnitude comparison . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>14|
|**12 **|**Fan  l**|**and Airflow Model**|**14**|
||12.1|Hot-side thermal resistance . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>15|
||12.2|Why the hot side matters to the cold side . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>15|
||12.3|Fan status in the present scope . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>15|
||12.4|Recirculating air, mass flow and an effectiveness representation . . . . . . . .|. . . . . . .<br>15|
||12.5|Advection, if gas sampling is continuous . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>16|
|**13 **|**Pelti**|**er Thermoelectric Model**|**16**|
||13.1|Cooling limits . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>16|
||13.2|Obtaining_α_,_Re_,_K_. Four routes . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>17|
||13.3|Unipolar versus bipolar drive . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . .<br>17|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

3 

|**14 Multi-Node Energy-Balance Equations**|**17**|
|---|---|
|14.1 Sign convention . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>17|
|<br>14.2 Derivation node by node<br>. . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>17|
|14.3 Comparison with the equations proposed in the specification . . . . . . . .|. . . . . . . .<br>18|
|14.4 Measurement node<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>18|
|**15 Nonlinear State-Space Model**|**18**|
|15.1 Compact form . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>18|
|15.2 Which temperature should be controlled?<br>. . . . . . . . . . . . . . . . . .|. . . . . . . .<br>19|
|**16 Linearised Model**|**19**|
|16.1 Jacobian structure . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>19|
|16.2 Why several operating-point models are needed . . . . . . . . . . . . . . .|. . . . . . . .<br>20|
|**17 Model Reduction**|**20**|
|**18 Thermal RC Equivalent**|**21**|
|18.1 Correspondence between the two representations<br>. . . . . . . . . . . . . .|. . . . . . . .<br>22|
|**19 Parameter Table**|**22**|
|**20 Numerical Parameter-Identification Plan**|**24**|
|**21 Open-Loop Simulation Plan and Dry-Run Results**|**25**|
|21.1 Time scales and stiffness . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>25|
|21.2 Experiments . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>26|
|21.3 Energy-balance verification . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>27|
|21.4 Actuator authority and the admissible setpoint range . . . . . . . . . . . . .|. . . . . . . .<br>28|
|**22 System Identification**|**28**|
|22.1 Operating-point dependence<br>. . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>28|
|<br>22.2 Step-response identification . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>29|
|**23 PID Design**|**30**|
|23.1 Structure . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>30|
|23.2 Tuning . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>30|
|**24 Fuzzy-PID Design**|**30**|
|24.1 Architecture . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>30|
|24.2 Normalisation . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>31|
|24.3 Membership functions and inference . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>31|
|24.4 Rule base and its reasoning . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>32|
|24.5 Known limitation of this rule base . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>32|
|**25 Multi-Setpoint Simulation**|**32**|
|25.1 Performance metrics<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>32|
|25.2 Reading the results . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>34|
|**26 Disturbance Simulation**|**34**|
|**27 Sensitivity Analysis**|**35**|
|27.1 Steady-state sensitivity . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>35|
|27.2 Dynamic sensitivity . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|. . . . . . . .<br>36|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

4 

|**28 **|**Uncertainty Analysis**|**37**|
|---|---|---|
||28.1 Sources and expected magnitudes<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>37|
||28.2 Recommended propagation method<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>37|
|**29 **|**MATLAB / Python Implementation Procedure**|**38**|
||29.1 Numerical integration . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>38|
||29.2 Choosing the step size<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>38|
||29.3 Code architecture . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>38|
||29.4 Symbolic, discrete and code form of every state equation . . . . . . . . . . . . . . . . .|.<br>39|
||29.5 Staged implementation plan<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>40|
||29.6 Notes for the embedded implementation . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>40|
|**30 **|**Experimental Validation Plan**|**41**|
||30.1 Principle . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>41|
||30.2 Metrics<br>. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>41|
||30.3 Validation matrix . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>41|
||30.4 Acceptance criteria . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>41|
||30.5 Instrumentation required . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>42|
|**31 **|**Research Decision Gates**|**42**|
|**32 **|**Final Mathematical Model**|**43**|
||32.1 Consolidated statement . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>43|
||32.2 Numerical implementation form . . . . . . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>43|
||32.3 The five model levels, and why they differ . . . . . . . . . . . . . . . . . . . . . . . . .|.<br>43|
|**33 **|**Recommended Next Experiments**|**44**|
|**34 **|**Research Contribution**|**44**|
|**35 **|**Recommended Experimental Parameters to Be Measured Before Numerical Calibration**|**45**|
|**36 **|**Conclusion**|**46**|
|**A **|**Answers to the Critical Engineering Questions**|**47**|
|**B**|**Reference Implementation**|**48**|
|**C **|**Note on Reference Selection and Verification**|**50**|



5 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

## **1 Problem Definition** 

### **1.1 Instrument context** 

The instrument is an E-Nose used as a dissolved gas analyser for transformer insulating oil. Its sensing front end comprises 14 metal-oxide semiconductor (MOS) gas sensors, one non-dispersive infrared (NDIR) sensor, and two SHT20 temperature/humidity sensors, mounted on a printed circuit board (PCB) at the base of a cylindrical chamber. A rectangular upper chamber above the cylinder circulates air cooled by a thermoelectric (Peltier) module. 

MQ/TGS gas sensors are operated with an internal heater and their conductance response depends strongly on the temperature of the sensing layer and on the temperature of the gas surrounding it [1, 2]. For a quantitative DGA application—where the goal is to regress gas concentrations rather than merely to classify oil condition—an uncontrolled chamber temperature introduces a slow drift term that is confounded with concentration [2]. MQ- and TGS-family devices have already been used successfully as the front end of E-Nose systems for transformer DGA [3–5], which makes their thermal environment a first-order design variable rather than a detail. Temperature stabilisation is therefore not a convenience: it is a precondition for a calibration model that transfers across days and across ambient conditions. 

### **1.2 Thermal-modelling objective** 

To derive and simulate a mathematical model describing heat generation, heat storage, conduction, convection, radiation and Peltier-based cooling in the E-Nose sensor chamber, and to use the validated model to design a Fuzzy-PID controller that maintains several chamber-temperature setpoints. 

### **1.3 Control objective** 



under disturbances from sensor heat generation, fan operation, ambient temperature change, Peltier operating point, thermal leakage through the chamber wall, and airflow variation. The values _Ti_ are not fixed in this document; Section 21.4 shows how the admissible set is bounded from below by the dew point and from above by the passive equilibrium temperature, and both bounds must be established experimentally. 

### **1.4 Method: derivation-first ordering** 

The analysis follows the sequence 

geometry _→_ assumptions _→_ heat generation _→_ energy balance _→_ ODEs _→_ parameters _→_ 

numerical model _→_ open loop _→_ identification _→_ PID _→_ Fuzzy-PID _→_ multi-setpoint _→_ validation _._ No step uses a result from a later step. In particular, the reduced-order plant model in Section 22 is extracted _from_ the physical model rather than postulated in advance. 

6 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

## **2 Geometric Interpretation** 

### **2.1 Given dimensions** 

Table 1: Chamber dimensions as specified. 

|Section|Quantity|Symbol|Value|
|---|---|---|---|
|A (cylinder)|outer diameter|_D_|116 mm|
||outer radius|_R_|58 mm|
||inner diameter|_d_|34 mm|
||inner radius|_r_|17 mm|
||height|_Hc_|29 mm|
|B (box)|length|_Lb_|60 mm|
||width|_Wb_|43 mm|
||height|_Hb_|54 mm|



### **2.2 The ambiguity of the 34 mm inner diameter** 

The specification gives an inner diameter without stating what it bounds. At least four physically distinct features are consistent with the number, and they lead to different models: 

#### **G1. Solid central body.** 

The bore is occupied by a solid object (a central pillar, a sensor holder, a gas-distribution cone). The free-air region is then a true annulus and _V_ cyl = _π_ ( _R_<sup>2</sup> _− r_<sup>2</sup> ) _Hc_ . The central body also adds thermal capacitance and a conduction path that the annular volume formula does not represent. 

#### **G2, open through-passage.** 

The bore is an open gas passage connecting the cylinder to the box. The air region is then the full cylinder, _V_ cyl = _πR_<sup>2</sup> _Hc_ , and the bore is the dominant convective coupling between the two sections rather than a void. 

#### **G3, inlet/outlet port.** 

The bore is a port of finite length in the top plate. Its volume is negligible but it fixes the airflow path and hence the convection coefficients. 

#### **G4, recess for the sensor array.** 

The bore is a step in the base plate under the PCB. It changes the wall area in contact with air and the sensor-to-wall conduction path. 

**Decision required (Gate 1).** The volume difference between G1 and G2 is 26.3 mL, about 6 % of the geometric volume, which is thermally unimportant (Section 27.2). The _airflow_ difference between them is not: G2 makes the bore the main path between the cooled box and the sensor cylinder, which changes _h_ by a factor that cannot be estimated from the numbers given. The interpretation must be resolved from the CAD model, not from the dimension list. 

Both G1 and G2 are carried forward. All subsequent volume results are reported for both. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

7 

## **3 Chamber Volume Calculation** 

### **3.1 Geometric volume** 

For the cylindrical section under the two interpretations, 





with the central core accounting for _V_ core = _πr_<sup>2</sup> _Hc_ = 26 _._ 33 mL. For the rectangular section, 



Hence 



### **3.2 Occupied volume and effective free-air volume** 

The three volumes must be kept distinct: 



The working estimate supplied with the specification is _V_ air _≈_ 397 mL. Taken together with the geometric volumes this implies 

Table 2: Implied occupied volume for the two geometric interpretations. 

|Interpretation|_V_geom(mL)<br>_V_occ =|_V_geom_−_397 mL|fill fraction|
|---|---|---|---|
|G1 (annulus)|419.47|22.47|5.4 %|
|G2 (full cylinder)|445.80|48.80|10.9 %|



**Consistency warning.** A fill fraction of 5.4 % is difficult to reconcile with a chamber that contains a populated PCB carrying 17 sensor packages, a finned cold-side heat exchanger, a fan and wiring. Either (i) the geometry is G2 or something between G1 and G2, (ii) the 397 mL estimate is optimistic, or (iii) part of the hardware sits outside the modelled envelope. This must be resolved before _V_ air is used, and the resolution is a measurement, not a calculation. 

### **3.3 How** _V_ **air should actually be determined** 

1. **CAD subtraction (preferred).** Build the assembly in the CAD tool, create a negative body of the internal cavity, subtract all populated components, and read the remaining volume. This also yields the internal _wetted area A_ , which the model needs and which no volume estimate provides. 

2. **Water or bead displacement.** Seal the ports, fill the fully populated chamber with a measured volume of liquid or of calibrated beads. Gives _V_ air directly with an uncertainty of a few millilitres. Only feasible if the electronics can be protected or a dummy build is available. 

3. **Gas dilution.** Inject a known volume of a tracer gas, measure the diluted concentration at equilibrium, and infer the volume. Non-destructive and uses hardware the laboratory already has for DGA work. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

8 

Method 1 is recommended because the same model yields _A_ int, _A_ ext and the component masses required in Section 8. 

**Result carried forward.** _V_ air is retained as a symbol. Where a number is needed for the dry-run, 397 mL<sup>[P]</sup> is used, and Section 27.2 shows that the choice is nearly irrelevant to the dynamics— which is itself the most useful outcome of this section. 

## **4 Thermal-System Boundary** 



<!-- Start of picture text -->
ambient<br>Gw∞<br>chamber wall Tw<br>Peltier module<br>u  =  I<br>Gaw<br>Gac<br>Gsw chamber air Ta cold side Tc<br>Gsa Qc, Qh T amb<br>sensors + PCB Ts hot side Th Gh∞<br>Q gen<br><!-- End of picture text -->

Figure 1: System boundary and heat-flow topology. Solid arrows are the thermal couplings retained in the five-node model. The Peltier module is a controlled two-port between the cold and hot nodes. 

**Inside the controlled volume.** MQ/TGS sensor array, NDIR sensor, SHT20 sensors, PCB, internal air. 

**Cooling subsystem.** Peltier module, cold-side heat exchanger, hot-side heatsink, fan. 

**Outside.** Ambient air, external wall surface, all downstream heat rejection. 

#### **System description.** 

Input _u_ : Peltier current _I_ A (or duty cycle mapped to current) 

States _x_ : [ _Ts, Ta, Tw, Tc, Th_ ]<sup>T</sup> , extended by the sensor node _Tm_ in Sec. 14.4 

Output _y_ : measured chamber temperature (SHT20) 

Disturbances _d_ : [ _T_ amb _, Q_ gen _, V_<sup>˙</sup> fan ]<sup>T</sup> 

Parameters : capacitances _Ci,_ conductances _Gij,_ Peltier ( _α, Re, K_ ) 

## **5 Assumptions** 

- **A1. Lumped nodes.** Each node is isothermal. Justified when the internal Biot number Bi = _hL_ char _/k_ solid _≪_ 1. For an aluminium wall of a few millimetres with _h ∼_ 10 W _/_ (m<sup>2</sup> K), Bi _∼_ 10<sup>_−_3</sup> , so the wall is safely lumped. For the PCB ( _k ≈_ 0 _._ 3 W _/_ (m K) through-plane) the assumption is weaker and is revisited in Gate 4. 

- **A2. Uniform air temperature.** Doubtful near individual MQ/TGS heaters, which run far above ambient. The model therefore predicts a mixed-mean air temperature, not the temperature at any particular point. Gate 7 tests this with two SHT20 units at different locations. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

9 

- **A3. Constant air properties.** _ρ_ , _cp_ , _k_ , _µ_ evaluated at 25 °C. Over 0 °C to 60 °C the density varies by about 20 %; because _Ca_ turns out to be dynamically irrelevant this is harmless. 

- **A4. All electrical power dissipated inside becomes heat.** Discussed in Section 6. 

- **A5. No mass exchange during a control interval.** Gas sampling flow is assumed either off or small enough that enthalpy advection is negligible. If sampling is continuous this assumption fails and an advection term must be added (Section 12). 

- **A6. Constant fan speed** in the present scope. Variable speed is treated as a disturbance, not as a second control input. 

- **A7. Peltier parameters** _α_ **,** _Re_ **,** _K_ **constant** with temperature. A first approximation only; real modules show 5 % to 15 % variation over a 50 K span, and incorporating the temperature dependence has been shown to improve model fidelity appreciably [6]. 

- **A8. One-dimensional heat paths** between nodes, represented by scalar conductances. 

## **6 Heat-Generation Model** 

### **6.1 Electrical power to thermal power** 

For each MQ/TGS sensor _i_ the instantaneous electrical power is 



where _Vs,i_ and _Is,i_ are the heater voltage and current. The sensing element contributes negligibly. Summing over the array, 



The second form is the practical one: MQ/TGS heaters are usually driven at constant voltage, and _Rh_ is itself temperature dependent, so _Q_ sens is a weak function of chamber temperature. This creates a positive feedback loop (hotter chamber _→_ different heater resistance _→_ different dissipation) that should be checked before being neglected. 

### **6.2 Total internal dissipation** 



_Q_ **sens** 

The dominant term. If the array uses pulsed heating or temperature modulation—common in E-Nose work and likely here given the 109-feature temporal pipeline, then _Q_ sens( _t_ ) is a periodic waveform, not a constant. Its _mean_ sets the operating point; its _amplitude and period_ determine whether it appears as a disturbance the controller must reject or is filtered out by the thermal mass. With a dominant time constant of a few hundred seconds, modulation faster than about 10 s will be strongly attenuated. Duty-cycled and pulsed heater operation is well documented for metal-oxide sensors [7, 8] and the schedule in use must be read from the firmware rather than assumed. This must be checked against the actual sensing cycle. 

#### _Q_ **NDIR** 

The IR source is pulsed. Duty cycle and lamp power required. 

#### _Q_ **PCB** 

Regulators, drivers and the microcontroller if they sit inside the chamber. Linear regulators dropping the heater supply can rival the heaters themselves and are easy to overlook. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

10 

#### _Q_ **fan,int** 

Shaft power of any fan inside the controlled volume. Nearly all of it ends up as heat in the air it moves. 

**Assumption A4 examined.** Electrical power entering a closed volume leaves only as heat, as radiation, or as chemical/optical work. For this instrument the optical output of the NDIR source is largely reabsorbed internally, and the chemisorption enthalpy at the MOS surfaces is many orders of magnitude below the heater power. Treating all internal electrical power as heat is therefore sound to better than 1 %. The exception is any power routed _out_ of the boundary through wiring, which must be excluded. 

### **6.3 Parameters required** 

Table 3: Heat-generation parameters and how to obtain them. 

|Quantity|Status|Source|Method|
|---|---|---|---|
|_Vh,i, Rh,i_ per MQ/TGS<br>sensor|[U]|sensor datasheet|cross-check with a 0.1<br>shunt<br>on the heater rail|
|Heater duty cycle|[U]|firmware|read the modulation schedule<br>from the Teensy firmware|
|_Q_NDIR|[U]|datasheet + duty|mean power over one measure-<br>ment cycle|
|_Q_PCB|[U]|measurement|total board current _×_ supply<br>voltage, heaters excluded|
|_Q_fan,int<br>_Q_gen total|[U]<br>[U]|measurement<br>measurement|fan supply current_×_voltage<br>**recommended:** single in-line<br>power<br>measurement<br>of<br>ev-<br>erything inside the boundary,<br>which avoids itemisation errors<br>entirely|



_Parameter not yet available; determine experimentally or obtain from the manufacturer._ For the dry-run a placeholder _Q_ gen = 9 W<sup>[P]</sup> is used. 

## **7 Sensor-Specific Thermal Considerations and Node Aggregation** 

### **7.1 Why the sensor array is thermally awkward** 

MQ and TGS devices are not passive components that happen to warm up. Each one contains a heater held at a high, actively regulated temperature, and the gas response is a function of that temperature. Three consequences follow: 

1. The array is simultaneously the _disturbance source_ and the _object being protected_ . Its own dissipation is the main thermal load the Peltier system must remove. 

2. Small changes in heater supply change both the dissipation and the sensing behaviour. A measured increase of only 3 mA in the heater current of an MQ device has been reported to shorten the recovery time by more than 150 s [1]; the heater rail is therefore a metrological parameter, not merely a powersupply detail. 

3. Ambient temperature and humidity shift the baseline of metal-oxide sensors substantially, and correction models are routinely required [2]. Thermal control is one way of removing the temperature part of that correction at the source. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

11 

### **7.2 Three candidate representations of the array** 

Table 4: Options for representing 14 heat-generating sensors. 

|Option|Description|Advantage|Cost / risk|
|---|---|---|---|
|A. One node|All 14 packages plus the PCB<br>lumped into a single _Ts_ with<br>_Q_gen = <sup>�</sup><br>_i _<sup>_Pi_</sup>|Five-state model; every pa-<br>rameter identifiable from one<br>heater-switching experiment|Cannot<br>represent<br>sensor-to-<br>sensor gradients, so it can-<br>not explain channel-to-channel<br>baseline spread|
|B. Groups|Sensors grouped by position<br>(e.g. inner ring, outer ring) or<br>by heater power|Captures the dominant spa-<br>tial gradient at modest cost|Requires at least one temper-<br>ature measurement per group,<br>otherwise the group capaci-<br>tances are unidentifiable|
|C. 14 nodes|Every sensor its own node|Resolves local hotspots and<br>sensor–sensor coupling|14+ states, _≥_28 unknown<br>parameters, none identifiable<br>without<br>14<br>thermocouples;<br>model complexity exceeds the<br>available measurements|



**Recommendation for the first stage: Option A.** The decisive argument is identifiability, not convenience. With the instrument as currently instrumented—two SHT20 units and no boardmounted thermometry—Options B and C contain parameters that no available measurement can determine, and an unidentifiable parameter acts as a free variable that absorbs modelling error without being detected. Option A is adopted, and the cost is stated explicitly: the model predicts a mean array temperature and says nothing about the difference between one sensor and its neighbour. That difference is measured, not modelled, in experiment E6 of Section 20, and if it proves large relative to the control tolerance, the model is upgraded to Option B, but not before. 

### **7.3 What is lost, quantitatively** 

Under Option A the sensor node carries the total capacitance _Cs_ =<sup>�</sup> _i_<sup>_micp,i_+</sup><sup>_m_PCB</sup><sup>_cp,_PCB and a single</sup> conductance to the air. The model therefore cannot reproduce: 

- the temperature difference between a sensor at the centre of the board and one at the edge, which is driven by the local airflow rather than by the mean; 

- thermal crosstalk, in which a sensor downstream of another receives preheated air; 

- the transient of an individual package during pulsed heating, whose time constant is far shorter than the array time constant. 

None of these affect the _control_ problem, which acts on the mean. All of them may affect the _sensing_ problem. Keeping the two questions separate is the reason Option A is defensible here and would not be defensible in a study of sensor response modelling. 

## **8 Thermal-Capacitance Model** 

For a lumped body, 



with _m_ the mass (kg) and _cp_ the specific heat capacity (J/(kg K)). For the air, 



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

12 

### **8.1 Numerical evaluation for air** 

With handbook values at 25 °C and 101.325 kPa ( _ρ_ air = 1 _._ 184 kg _/_ m<sup>3</sup> , _cp,_ air = 1005 J _/_ (kg K)) and _V_ air = 397 mL: 



**Structural result 1.** The entire air content of the chamber stores 0.47 J per kelvin. A single 60 g aluminium heatsink stores about 54 J/K; the populated PCB stores tens of J/K. The air is therefore _not_ a thermal storage element at all—it is a coupling medium. Two consequences follow immediately and shape the rest of this report: 

1. The air node has a time constant of order 0.1 s (Section 21.1), which makes the system numerically stiff and justifies a quasi-steady reduction of _Ta_ . 

2. “Controlling the chamber air temperature” physically means controlling the temperature of the surfaces bounding the air. The air follows within a fraction of a second. 

### **8.2 Solid capacitances** 

Table 5: Thermal capacitances. No solid mass has been measured; the placeholder column exists only to permit the dry-run. 

|Node|Composition|Symbol|Dry-run<sup>[P] </sup>(J/K)|How to determine|
|---|---|---|---|---|
|Sensor + PCB|FR4, copper, packages|_Cs_|40|weigh the populated board; _cp ≈_<br>1100 J_/_(kg K)for FR4,385for cop-<br>per; or identify from a step test|
|Air|enclosed gas|_Ca_|0.472|Eq. (11), derived|
|Wall|chamber body|_Cw_|150|weigh<br>the<br>enclosure;<br>_cp_<br>_≈_<br>900 J_/_(kg K) (Al), 1500 (typical<br>filled polymer) — **the material**<br>**must be stated**|
|Cold side|TEC ceramic + cold-side heat exchanger|_Cc_|25|weigh|
|Hot side|TEC ceramic + heatsink|_Ch_|90|weigh|



### **8.3 Effect on response time** 

For a single node exchanging with its surroundings through a total conductance _G_ , the time constant is _τ_ = _C/G_ . Chamber volume enters only through _Ca_ , and _Ca_ is negligible; the response time of the instrument is therefore set by the wall and board masses. This is quantified in Section 27.2, where a 20 % change in _Cw_ shifts the dominant time constant by 35 s while a 20 % change in _Ca_ shifts it by 0.05 s. 

## **9 Conduction Model** 

Fourier conduction through a slab of thickness _L_ and area _A_ : 



The convective counterpart is 



Lumped-parameter thermal networks of this kind are standard practice for electronic assemblies and are accurate enough for control-oriented work provided the topology and the parameters are calibrated against 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

13 

measurement [9–11]. Series and parallel paths combine as in an electrical network, 



Relevant conduction paths in this instrument: 

1. PCB _→_ mounting standoffs _→_ chamber base ( _Gsw_ ). Often the dominant escape route for sensor heat, and frequently omitted from simple models. 

2. Cold-side assembly _→_ Peltier ceramic _→_ hot plate: the back-conduction _K_ in the thermoelectric model, not a separate term. 

3. Wall thickness itself: for a metal wall of a few millimetres, _R_ cond _∼_ 10<sup>_−_3</sup> K _/_ W, negligible against the surface resistances of order 1 K/W. The wall is therefore modelled as a single isothermal node with surface resistances on both faces. 

4. Thermal interface material at every TEC face. A 0.1 mm grease layer over 40 _×_ 40 mm contributes about 0.06 K/W per interface. Small but not zero, and it degrades with age. 

## **10 Convection Model** 

### **10.1 General form** 



with _h_ obtained from a Nusselt correlation, 

### **10.2 Three candidate models** 

**A. Natural convection.** With the Rayleigh number 



_L_ = _Hc_ = 29 mm, ∆ _T_ = 5 K, _β_ = 1 _/T ≈_ 3 _._ 36 _×_ 10<sup>_−_3</sup> _/_ K gives Ra _≈_ 1 _._ 2 _×_ 10<sup>4</sup> : laminar. A horizontal-plate correlation Nu = 0 _._ 54 Ra<sup>1</sup><sup>_/_4</sup> then yields 



**B. Forced convection.** For the rectangular section the hydraulic diameter is 



With an _assumed_ face velocity of 1 m/s<sup>[P]</sup> , Re = 3 _._ 2 _×_ 10<sup>3</sup> , transitional, not fully turbulent. Applying Dittus–Boelter (Nu = 0 _._ 023 Re<sup>0</sup><sup>_._8</sup> Pr<sup>0</sup><sup>_._4</sup> ) outside its strict validity range gives an order-of-magnitude estimate only: 



**C. Mixed convection.** Since _h_ nc and _h_ fc are within 30 % of each other, buoyancy and forced flow are comparable and neither may be ignored. A standard blending, 



gives _h ≈_ 7 _._ 7 W _/_ (m<sup>2</sup> K). 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

14 

**Recommendation for the first simulation.** Use a single lumped surface conductance _G_ = _h_ tot _A_ with _h_ tot = _h_ mixed + _h_ rad treated as _one identified parameter per interface_ , rather than computing _h_ and _A_ separately. The correlations above are only good to a factor of two in this geometry; a step test identifies the product _hA_ to a few percent. Correlations are used here to establish the order of magnitude and to decide which mechanisms matter, not to supply final numbers. 

_Fan airflow velocity: parameter not yet available; determine experimentally (hot-wire anemometer at the duct exit, or fan curve intersected with the estimated system impedance)._ 

## **11 Radiation Model** 

Between two grey surfaces, 



For small temperature differences this linearises to an equivalent coefficient 



### **11.1 Order-of-magnitude comparison** 

At _Tm_ = 303 K: 



Table 6: Radiation versus convection for plausible internal surface finishes. 

|Internal surface|_ε_|_h_rad(W/(m<sup>2 </sup>K))|share of total surface exchange|
|---|---|---|---|
|Anodised / painted / plastic|0.90|5.69|46 % (forced), 53 % (natural)|
|Mill-finish aluminium|0.20|1.26|16 %|
|Polished aluminium|0.05|0.32|4 %|



**Structural result 2.** Radiation should not be neglected without first checking the surface finish. For a high-emissivity internal finish it carries roughly half of the surface heat exchange in this chamber, because the convection coefficients are themselves small (5 W/(m<sup>2</sup> K) to 8 W/(m<sup>2</sup> K)) in a small enclosure at low air speed. The correct treatment is: 

1. retain _εσA_ ( _T_ 1<sup>4</sup><sup>_−T_4</sup> 2<sup>)intheextendedmodel;</sup> 

2. in the reduced model, fold radiation into the surface conductance as _h_ tot = _h_ conv + _h_ rad, which is exact to first order over a 20 K span; 

3. declare the internal surface finish (Gate 6). If _ε <_ 0 _._ 1 the term may then be dropped with a stated 4 % error. 

Note that the linearisation makes _h_ rad temperature dependent ( _∝ Tm_<sup>3):between10 °Cand50 °Cit</sup> varies by about 40 %, contributing to the plant nonlinearity. 

## **12 Fan and Airflow Model** 

The fan affects the model through four coupled channels: air velocity _v_ , Reynolds number, convection coefficient, and hot-side heat rejection. 

15 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

### **12.1 Hot-side thermal resistance** 

The steady relation for the heatsink is 



Manufacturer data for forced-convection heatsinks follow approximately [12, 13] 



so a halving of face velocity raises _R_ th by 40 % to 75 %. In the open-loop study (Experiment F) airflow is varied by scaling both _Gac_ and _Gh∞_ by _s_<sup>0</sup><sup>_._8</sup> . 

### **12.2 Why the hot side matters to the cold side** 

_R_ th,hs does not merely affect the hot node. Through the back-conduction term _K_ ( _Th − Tc_ ) it directly reduces the available cooling: raising _Th_ raises _Tc_ for the same current. A degraded fan therefore appears at the controlled variable as a loss of actuator authority, not as a temperature offset, a failure mode worth instrumenting. 

### **12.3 Fan status in the present scope** 

Fan speed is treated as **constant** . Variable-speed operation would add a second control input and make the plant two-input; this is noted as a possible extension but is outside the present model. If the fan is switched by the firmware during a measurement cycle, it becomes a _measured disturbance_ and should be fed forward. 

### **12.4 Recirculating air, mass flow and an effectiveness representation** 

The air loop carries heat from the sensor chamber to the cold-side exchanger. Its mass flow rate is 



and the heat it transports between two stations at temperatures _T_ 1 and _T_ 2 is 



If the cold-side exchanger is treated as a compact heat exchanger with effectiveness _ε_ hx _∈_ [0 _,_ 1], the fraction of the maximum thermodynamically available temperature drop that is actually realised. The cooling delivered to the recirculating air becomes 



**This is a modelling assumption, and it is the same assumption already made.** Comparing Eq. (30) with the conductance form used in Eq. (40) shows 



so the effectiveness formulation and the lumped-conductance formulation are the same model written twice. The choice between them is practical: _Gac_ is identifiable from a single step test, whereas _ε_ hx and _m_ ˙ are separately identifiable only if the airflow is measured. The conductance form is therefore used throughout, and Eq. (30) is retained because it says what _Gac_ is made of: an airflow term the fan controls and an exchanger term the geometry fixes. An upper bound follows immediately — _Gac ≤ mc_ ˙ _p,_ air, which is a useful sanity check on any identified value. 

16 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

### **12.5 Advection, if gas sampling is continuous** 

If sample gas flows through the chamber at volumetric rate _V_<sup>˙</sup> , an enthalpy term must be added to the air balance: 



Its equivalent conductance is _G_ adv = _ρcpV_<sup>˙</sup> . For _V_<sup>˙</sup> = 200 mL _/_ min this is 4.0 × 10<sup>−3</sup> W/K, negligible against _Gac_ ; for 5 L/min it is 0.10 W/K, no longer negligible. _The sampling flow rate must be stated before this term can be dismissed._ 

## **13 Peltier Thermoelectric Model** 

The Peltier module, also called a thermoelectric cooler (TEC), is not adequately represented by a constant cooling power, because the cooling delivered by a Peltier module depends on its own terminal temperatures — which is precisely the feedback path that shapes the closed loop. The standard one-dimensional model is used [6, 14]: 





with _Tc_ , _Th_ in **kelvin** . Term by term: 

**Peltier pumping** _αITc_ **.** 

Heat absorbed at the cold junction, proportional to current and to absolute cold-side temperature. This is the useful effect and it is _bilinear_ in ( _I, Tc_ ), the primary source of plant nonlinearity. 

**Joule heating** 2<sup><u>1</u></sup><sup>_I_2</sup><sup>_Re_</sup><sup>**.**</sup> 

Ohmic dissipation, split equally between the junctions. It subtracts from cooling and grows quadratically, which is why maximum current is not maximum cooling. 

**Back conduction** _K_ ( _Th − Tc_ ) **.** 

Heat leaking through the module against the pumping direction. It sets the maximum achievable ∆ _T_ . 

#### **Hot-side rejection.** 

_Qh_ must be removed by the heatsink, Eq. (26). Note the exact energy identity 



which the numerical implementation must satisfy to machine precision. This is used as a codecorrectness test in Section 21. 

### **13.1 Cooling limits** 

Setting _Qc_ = 0 in Eq. (33) gives the maximum temperature difference at a given current, and maximising over _I_ gives the familiar 



These expressions are the quickest sanity check on any identified parameter set: if ∆ _T_ max comes out at 200 K the parameters are wrong. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

17 

### **13.2 Obtaining** _α_ **,** _Re_ **,** _K_ **. Four routes** 

1. **From datasheet maxima.** Given _I_ max, _V_ max, _Qc,_ max, ∆ _T_ max at hot-side temperature _Th_ : 



This is the fastest route and is usually good to 10 % to 20 %. 

2. **Manufacturer performance curves.** A systematic comparison of the competing extraction formulae, together with their hidden assumptions, is given by Wan _et al._ [14] and should be consulted before any one formula is trusted. Read _Qc_ at several ( _I,_ ∆ _T_ ) pairs and fit Eqs. (33)–(34) by least squares. 

3. **Direct electrical measurement.** _Re_ from a fast AC or short-pulse measurement (a DC measurement includes the Seebeck back-EMF and is wrong); _α_ from the open-circuit voltage under an imposed ∆ _T_ ; _K_ from a steady-state heat-flow measurement with _I_ = 0. 

4. **Parameter identification in situ.** Fit ( _α, Re, K_ ) together with the surrounding conductances to a set of steady-state ( _I, Tc, Th, Ta_ ) measurements taken on the assembled instrument. This is the recommended route because it absorbs interface resistances and mounting imperfections that the datasheet cannot know about. 

_Peltier parameters: not yet available; obtain from the manufacturer or by identification. The dry-run uses α_ = 0 _._ 053 V _/_ K<sup>_[P]_</sup> _, Re_ = 1 _._ 90 Ω<sup>_[P]_</sup> _, K_ = 0 _._ 62 W _/_ K<sup>_[P]_</sup> _._ 

### **13.3 Unipolar versus bipolar drive** 

If the module is driven by a single-quadrant supply, _I ≥_ 0 and the actuator can only cool. The reachable set of steady-state chamber temperatures is then bounded above by the passive equilibrium _Ta_ ( _I_ = 0) and below by _Ta_ ( _I_ max). Section 21.4 quantifies this. Bipolar drive (an H-bridge) permits heating and makes setpoints above the passive equilibrium reachable, at the cost of a sign discontinuity in the plant gain that the controller must handle. 

## **14 Multi-Node Energy-Balance Equations** 

### **14.1 Sign convention** 

For every node, the first law is written as 



where _Q_<sup>˙</sup> _j→i >_ 0 denotes heat _entering_ node _i_ . A conductive or convective coupling between nodes _i_ and _j_ contributes + _Gij_ ( _Tj − Ti_ ) to node _i_ and _−Gij_ ( _Tj − Ti_ ) to node _j_ ; the pair is therefore antisymmetric and the sum of all coupling terms over all nodes is identically zero. Only the external terms ( _Q_ gen, exchanges with _T_ amb, and the Peltier work input) change the total energy. This is the property that the numerical energy-balance test exploits. 

### **14.2 Derivation node by node** 

**Node 1, sensors and PCB.** Heat generated internally leaves by convection to the air and by conduction to the wall through the standoffs: 



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

18 

**Node 2, chamber air.** The air receives heat from the sensors and passes it to the wall and to the cold-side exchanger: 



**Node 3, chamber wall.** The wall is heated by the air and by conduction from the board, and loses heat to ambient: 



**Node 4, Peltier cold side.** The cold assembly absorbs heat from the air and is emptied by the thermoelectric pumping: 



**Node 5. Peltier hot side.** The hot assembly receives the pumped heat plus the electrical work and rejects it to ambient: 



### **14.3 Comparison with the equations proposed in the specification** 

The specification’s draft equations are recovered from Eqs. (39)–(43) with two differences, both deliberate: 

1. **The term** _Gsw_ ( _Ts − Tw_ ) **is added.** The draft routes all sensor heat through the air. Physically the PCB is bolted to the enclosure, and for a metal chamber this conduction path is often comparable to, or larger than, the convective path. Omitting it biases the identified _Gsa_ upward and misattributes the heat flow. 

2. **Sign of** _Qc_ **in the cold-node equation confirmed.** With Eq. (33) as written, _Qc >_ 0 means heat is _removed from_ the cold assembly, so it enters Eq. (42) with a minus sign, exactly as in the draft. At _I_ = 0, _Qc_ = _−K_ ( _Th − Tc_ ) _<_ 0 when _Th > Tc_ , which correctly makes the cold node _gain_ heat from the hot side by conduction. The equation set is therefore valid at zero current, which is a necessary check. 

### **14.4 Measurement node** 

The controller does not see _Ta_ ; it sees an SHT20 reading. The sensor has a finite response time in still or slowly moving air, and the firmware samples it at a fixed interval. Both are represented by a first-order lag: 



This node carries no heat and does not affect the thermal balance, but it is essential to the control design: it is the only source of phase lag in an otherwise minimum-phase plant, and it determines how aggressively the loop may be tuned. _τm not yet available; determine by a step test against a fast reference thermocouple. The dry-run uses τm_ = 15 s<sup>_[P]_</sup> _._ 

## **15 Nonlinear State-Space Model** 

### **15.1 Compact form** 

With 



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

19 

the model is 



The nonlinearity is confined to _Qc_ and _Qh_ and is of two kinds: bilinear ( _uI x_ 4, _u x_ 5) and quadratic ( _u_<sup>2</sup> ). Everything else is linear. The fan enters through _Gac_ and _Gh∞_ , i.e. as a parameter-affine disturbance. 

### **15.2 Which temperature should be controlled?** 

The specification asks whether _y_ = _Ta_ or _y_ = _Ts_ is the right controlled variable. The choice is not cosmetic: 

||**Control**_Ta_**(chamber air)**|**Control**_Ts_ **(sensor/PCB)**|
|---|---|---|
|Measurable?|Yes, directly by the SHT20 already fit-<br>ted|Only indirectly; would need a thermis-<br>tor bonded to the board|
|Physically meaningful|Partly: it is the gas temperature, which|More: MQ/TGS response depends on|
|for gas sensing?|sets density and diffusion|the sensing-layer temperature, which<br>tracks the package|
|Controllability|Strong: the air is tightly coupled to the<br>cold-side assembly (_Gac_dominates)|Weaker: _Ts_is separated from the actu-<br>ator by two more thermal resistances|
|Dynamics|Very fast node, essentially algebraic|Slow node,_τs ≈_89 sin the dry-run|



**Recommendation.** Control _Ta_ as measured by the SHT20 (i.e. _y_ = _Tm_ ), because it is the variable that actually exists in the instrument and the one with the strongest actuator coupling. But _monitor Ts_ , and report the gradient _Ts − Ta_ with every measurement: with the dry-run parameters that gradient is 20 K to 25 K, is a strong function of _Q_ gen, and is what shifts the sensor baseline. A cascade structure (outer loop on _Ts_ , inner loop on _Ta_ ) is the natural upgrade once _Ts_ can be measured, and should be flagged as future work rather than attempted now. 

## **16 Linearised Model** 

### **16.1 Jacobian structure** 

Around an equilibrium ( _x_ 0 _, u_ 0 _, d_ 0) satisfying _f_ ( _x_ 0 _, u_ 0 _, d_ 0) = 0, write _δx_ = _x − x_ 0 and 



The thermoelectric partial derivatives, obtained directly from Eqs. (33)–(34), are 



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

20 



Hence the two rows of _A_ that carry the nonlinearity are 



and the input matrix is 



**Structural result 3, a stability condition on the hot side.** _A_ 55 _>_ 0, i.e. an unstable hot node, occurs when 



Physically: above this current the extra heat delivered to the hot side by a rise in _Th_ exceeds what the heatsink can remove, and the module runs away. With the dry-run values the threshold is 65.7 A, far above _I_ max = 4 A, so the design is safe, but the condition should be re-evaluated once the real _K_ and _R_ th,hs are known, because it is _Gh∞_ , the parameter most likely to degrade in service, that protects it. 

### **16.2 Why several operating-point models are needed** 

_A_ and _B_ both depend on _I_ and on the equilibrium temperatures. The consequences are quantified in Section 22: the DC gain from current to chamber temperature falls from about −14 K/A at 0.5 A to −4.8 K/A at 3 A. A controller tuned at one end of that range is either sluggish or oscillatory at the other. The same trend, a steady-state gain that falls as current rises, has been reported for Peltier modules elsewhere [15]. This, not a preference for fuzzy methods. Is the engineering justification for gain adaptation. 

## **17 Model Reduction** 

Reduction is performed after the full model exists, and only where the evidence supports it. The evidence used here is the eigenvalue spectrum of Section 21.1, the two sensitivity rankings of Sections 27–27.2, and the identification residuals of Section 22. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

21 

Table 7: Reduction path with the justification and the cost of each step. 

|Model|Step and justification|Retained|Lost|
|---|---|---|---|
|5 nodes +_Tm_|Reference<br>model<br>derived<br>in<br>Sec. 14|everything|spatial gradients; individual sen-<br>sors|
|4 nodes|Eliminate_Ta_by quasi-steady re-<br>duction: _τa_ = 0_._15 sis1_._7_×_10<sup>3</sup><br>times faster than the dominant<br>mode, and _Ca_ changes _τ_dom by<br>0.02 %|all steady-state behaviour, all<br>dynamics slower than 1 s|the sub-second air transient,<br>which no SHT20 can measure|
|3 nodes|Merge_Tc_ and_Th_ only if the hot<br>side is stiff, i.e._Gh∞≫K_ and<br>_Ch_ small. **Not justified here**: _Th_<br>rises by17 Kbetween_I_ = 0and<br>_I_ = 4 Aand feeds back through<br>_K_(_Th −Tc_)|—|the loss of cooling capacity as<br>the hot side warms — the domi-<br>nant nonlinearity|
|2 poles + 1<br>zero|Control model only, fitted to<br>the step response with RMSE<br>0.035 K (Sec. 22)|the input–output behaviour<br>near one operating point|all internal temperatures; valid-<br>ity away from that operating<br>point|



**Reduction is not monotone.** The 5 _→_ 4 step is free: nothing measurable is lost. The 4 _→_ 3 step is refused on physical grounds even though it would be convenient, because merging the Peltier faces deletes precisely the mechanism that makes the plant nonlinear. The final 4 _→_ 2 step is accepted only for controller tuning and is explicitly labelled a control model (level E of Section 30.3), never a description of the chamber. 

## **18 Thermal RC Equivalent** 

Table 8: Thermal–electrical analogy used throughout. 

|Thermal quantity|Electrical analogue|Unit|
|---|---|---|
|Temperature_T_<br>|voltage|K_↔_V|
|Heat flow <sup>˙</sup>_Q_|current|W_↔_A|
|Thermal resistance_R_th|resistance|K/W|
|Thermal capacitance_C_|capacitance|J/K|
|Heat source_Q_gen|current source|W|
|Ambient_T_amb|ground reference|K|
|Peltier|_controlled_current source (two-port)|W|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

22 



<!-- Start of picture text -->
R aw<br>Q gen controlled source<br>Qc, Qh ( I )<br>Tw Ts Ta Tc Th<br>R sw R sa R ac TEC<br>Cw Cs Ca Cc Ch<br>R w∞ R h∞<br><!-- End of picture text -->

_T_ amb 

Figure 2: Thermal RC network equivalent to Eqs. (39)–(43). Each node capacitance is referenced to ambient; _Rij_ = 1 _/Gij_ . The Peltier module is not a resistor but a current-controlled two-port that removes _Qc_ from node _Tc_ and injects _Qh_ = _Qc_ + _P_ elec into node _Th_ . 

### **18.1 Correspondence between the two representations** 

Applying Kirchhoff’s current law at each node of Figure 2 reproduces Eqs. (39)–(43) line for line. The network form is useful for three things the differential equations do not make obvious: 

1. **Series/parallel reduction.** The passive path from air to ambient is the parallel combination 



which with the dry-run values gives 0.423 + 0.222 = 0.645 W/K. 

This number must not be used directly to predict the air temperature, and the reason is instructive. _G_ pass is the conductance seen _from the air node_ , but not all of the generated heat passes through the air node: the board also conducts directly to the wall through _Gsw_ . At _Q_ gen = 9 W the model gives 5.74 W leaving the sensor node through the air and 3.26 W going straight to the wall, so 36 % of the load bypasses the air. Applying ∆ _T_ = _Q_ gen _/G_ pass would give 13.9 K and a chamber temperature of 38.9 °C, whereas the full model gives 36.1 °C. The simple reduction overestimates the rise by 2.8 K. The measurable quantities are the _apparent_ conductances 



both obtainable with a thermometer and a power meter. The gap between 0.808 and the network value 0.645 W/K is itself informative: it measures how much heat leaves the board without passing through the air, and therefore how large _Gsw_ is. 

2. **Dominant resistance identification.** The largest resistance in a series path is the bottleneck. Here 1 _/K_ = 1 _._ 61 K _/_ W dominates the TEC path, meaning the module’s own conduction, not the heatsink, limits passive heat removal. 

3. **Direct mapping to a circuit simulator** if a co-simulation with the electronics is ever wanted. 

## **19 Parameter Table** 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

23 

Table 9: Complete parameter inventory. Status: D = derived, M = measurable directly, P = placeholder used for the dry-run only, U = unknown, no value assigned. Sensitivity rank from Sections 27–27.2 (H = high, M = medium, L = low). 

|Parameter|Symbol|Unit|Physical meaning|Status|Source / method of determina-<br><br>|
|---|---|---|---|---|---|
||||||tion<br>(Sens.)|
|**Geometry**||||||
|Outer radius|_R_|mm|cylinder outer radius|D (58)|specification<br>(L)|
|Inner radius|_r_|mm|bore radius, meaning|D (17)|specification; **resolve G1/G2**|
||||unresolved||**from CAD**<br>(L)|
|Cylinder height|_Hc_|mm||D (29)|specification<br>(L)|
|Box dimensions|_Lb_<br>_Wb_|mm|upper chamber|D<br>(60/43/54)|specification<br>(L)|
||_Hb_|||||
|Geometric|_V_geom|mL|envelope volume|D (419.5 /|Sec. 3<br>(L)|
|volume||||445.8)||
|Occupied volume|_V_occ|mL|solid components|U|CAD subtraction<br>(L)|
|Free-air volume|_V_air|mL|effective gas volume|P (397)|CAD or displacement<br>(L)|
|Internal<br>wetted|_A_int|m<sup>2</sup>|<br>convection area|<br>U|<br><br>CAD<br>(H)|
|area||||||
|External area|_A_ext|m<sup>2</sup>|wall-to-ambient area|U|CAD<br>(M)|
|**Air properties**(2|5 °C, 101.|325 kPa)||||
|Density|_ρ_air|kg/m<sup>3</sup>||D (1.184)|handbook<br>(L)|
|Specific heat|_cp_air|J/(kg K)||D (1005)|handbook<br>(L)|
|i<br>Conductivity|_,_<br>_k_air|<br>W/(m K)||<br>D<br>(0.0263)|handbook<br>(L)|
|Dynamic viscos-|_µ_air|Pa s||D|handbook<br>(L)|
|ity||||(1_._849 _×_<br>10<sup>_−_5</sup>)||
|Prandtl number|Pr|–||D (0.707)|handbook<br>(L)|
|**Thermal capacita**|**nces**|||||
|<br>Sensor + PCB|<br>_Cs_|J/K|board thermal mass|P (40)|weigh board; or identify from<br>step test<br>(H)|
|Air|_Ca_|J/K|gas thermal mass|D (0.472)|<br> Eq. (11)<br>(L)|
|Wall|_Cw_|J/K|enclosure<br>thermal<br>mass|P (150)|weigh enclosure, state material<br>(**H**)|
|Cold assembly|_Cc_|J/K|cold-side assembly +|P (25)|weigh<br>(L)|
||||exchanger|||
|Hot assembly|_Ch_|J/K|hot plate + heatsink|P (90)|weigh<br>(L)|
|**Thermal conduct**|**ances**(_G_|= 1_/R_th)||||
|Sensor_→_air|_Gsa_|W/K|convection + radiation|P (0.30)|step test with known_Q_gen (M)|
|Sensor_→_wall|_Gsw_|W/K|standoff conduction|P (0.15)|step test; or CAD +_k_<br>(L)|
|Air_→_wall|_Gaw_|W/K|internal<br>surface<br>ex-|P (0.40)|step test<br>(M)|
||||change|||
|Air_→_cold-side<br>assembly|_Gac_|W/K|cold-side<br>heat<br>ex-<br>changer|P (2.50)|step test at several currents (M)|
|Wall_→_ambient|_Gw∞_|W/K|external losses|P (0.50)|cool-down test,_I_ = 0<br>(**H**)|
|Heatsink_→_ambien|t_Gh∞_|W/K|1_/R_th,hs|P (2.86)|heatsink datasheet at measured<br>airflow then verif<br>(M)|
|**Peltier module**|||||l,  y<br>|
|<br>Seebeck<br>coeffi-|_α_|V/K|module effective value|P (0.053)|datasheet maxima or open-|
|i<br>cient|||||circuit test<br>(**H**)|
|Electrical<br>resis-|_Re_||module resistance|P (1.90)|<br><br>AC or pulsed measurement (M)|
|tance||||||
|Thermal conduc-|_K_|W/K|back conduction|P (0.62)|datasheet or _I_ = 0 heat-flow|
|tance|||||test<br>(**H**)|
|Max current|_I_max|A|actuator saturation|P (4.0)|driver rating<br>(M)|
||||||<br><br>_continued on next page_|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

24 

|Parameter|Symbol|Unit|Physical meaning|Status|Source / method<br>(Sens.)|
|---|---|---|---|---|---|
|**Heat generation a**<br><br>|**nd envir**<br>|**onment**<br>||||
|MOS<br>heater|_Ps,i_|W|per sensor|U|datasheet + shunt measurement|
|power|||||(**H**)|
|Total<br>internal|_Q_gen|W|Eq. (9)|P (9)|single in-line power measure-|
|power|||||ment<br>(**H**)|
|Ambient tempera-|_T_amb|°C|environment|M|external sensor; log during ev-|
|ture|||||ery run<br>(**H**)|
|Fan face velocity|_v_<br>|m/s|airflow|U|hot-wire anemometer<br>(M)|
|Sampling flow|˙_V_|L/min|gas flow through cham-<br>ber|U|flow meter; decides if advection<br>matters<br>(L)|
|**Measurement and**|**control**|||||
|Sensor lag|_τm_|s|SHT20 response|P (15)|step test vs. fast thermocouple<br>(M)|
|Sample time|_Ts_|s|control interval|D (1.0)|firmware setting, see Sec. 29<br>(M)|
|Emissivity|_ε_|–|internal surface|U|surface finish specification (M)|



Fourteen of the parameters above have no value. The three that matter most and are cheapest to obtain are _Q_ gen (one power measurement), _Gw∞_ (one cool-down curve at _I_ = 0), and the Peltier triplet ( _α, Re, K_ ) (one datasheet). Together these fix the steady-state behaviour almost completely. 

## **20 Numerical Parameter-Identification Plan** 

The parameters are not all identifiable from the same experiment. The following sequence isolates them, each step using only what the previous steps established. 

25 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

Table 10: Identification sequence. Each experiment is designed so that the target parameters dominate the observed response. 

|#|Experiment|Identifies|Procedure|
|---|---|---|---|
|E1|Power audit, chamber sealed,<br>fan and TEC off|_Q_gen|Measure total supply current and voltage of<br>everything inside the boundary over one full<br>sensing cycle; report mean and peak.|
|E2|Passive warm-up from ambi-<br>ent,_I_ = 0, fan on|_G_pass,_τ_dom,_Cw_|Log _Ta_, _Ts_, _Tw_, _T_amb to steady state. The<br>apparent conductance_Q_gen_/_∆_T∞_follows di-<br>rectly; note that it exceeds the air-node net-<br>work value because part of the load bypasses<br>the air (Sec. 16.1). The dominant time con-<br>stant gives_C_eff.|
|E3|Passive cool-down, power<br>off|_Gw∞_,_Cw_ separately|Exponential fit of_Tw_ decay with no internal<br>source removes the_Q_gen uncertainty.|
|E4|TEC steady-state sweep,_I ∈_<br>_{_0_._5_,_1_, . . . , I_max_}_|_α, Re, K_,_Gac_,_Gh∞_|At each current record steady_Ta, Tc, Th, T_amb<br>and electrical power.<br>Fit Eqs. (33)–(34)<br>jointly by least squares.|
|E5|Current step at several oper-<br>ating points|dynamics,_Cc_,_Ch_|Step_I_ by_±_0_._5 Aaround each point; fit the<br>transient.|
|E6|Load step, sensor heaters<br>switched|_Gsa_,_Gsw_,_Cs_|Switch the heater array on/off with the TEC<br>at fixed current; the_Ts_ transient separates the<br>board from the air.|
|E7|SHT20 step response|_τm_|Move the sensor between two stirred baths,<br>or step the chamber and compare with a fine<br>thermocouple.|
|E8|Airflow variation|_n_in_R_th(_v_)|Repeat E4 at two or three fan voltages.|



**Identifiability caution.** _Gsa_ and _Gsw_ appear only as a sum in the steady-state _Ts_ balance; they are separable only through the transient, and only if _Cs_ is known independently or the wall temperature is measured. If the wall is not instrumented, merge them into a single _Gs→_ rest and say so. Reporting two separately identified numbers that the data cannot distinguish is worse than reporting one. 

## **21 Open-Loop Simulation Plan and Dry-Run Results** 

Everything from here to Section 28 is executed with the placeholder parameter set of Section 19. The purpose is to verify that the equations, the code and the design workflow are correct and that the predicted behaviour is physically sensible. **These curves are not predictions of the instrument’s behaviour** and no number from them should be quoted as a property of the hardware. 

### **21.1 Time scales and stiffness** 

Single-node estimates _τi_ = _Ci/_<sup>�</sup> _j_<sup>_Gij_give</sup> 



The eigenvalues of the linearised system at _I_ = 2 A correspond to 



where 15.0 s is the (decoupled) measurement node. The stiffness ratio is 253 _._ 2 _/_ 0 _._ 146 _≈_ 1 _._ 7 _×_ 10<sup>3</sup> . 

26 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

**Consequence for the numerics.** An explicit solver must resolve the fastest mode. For the classical fourth-order Runge–Kutta method (RK4) the stability bound is _|λ|h <_ 2 _._ 78, i.e. _h <_ 2 _._ 78 _τa_ = 0 _._ 41 s. This was confirmed: integration with _h_ = 0 _._ 5 s diverges, while _h ∈{_ 0 _._ 4 _,_ 0 _._ 2 _,_ 0 _._ 1 _,_ 0 _._ 05 _}_ s agree on the final state to 1 × 10<sup>−14</sup> . A step of _h_ = 0 _._ 1 s is used throughout, giving four significant figures of margin at acceptable cost. Steady states are computed by Newton solution of _f_ ( _x_ ) = 0 rather than by time-marching, for the same reason. 

### **21.2 Experiments** 



<!-- Start of picture text -->
A. Qgen= 0, | = 0 (consistency test) B. Qgen=9 W,1=0<br>—nr re 55<br>26.0 —h —h<br>—w 50<br>o Co<br>L255 Las<br>2 2 —nr er<br>2 25.04 —— 2 40 bh —th<br>H g —1<br>& 24.5 E35<br>30<br>24.0<br>25<br>° 10 20 30 40 50 ° 10 20 30 40 50<br>time [min] time [min]<br>C. T, vs internal dissipation (/ = 0) D. Tz vs Peltier current<br>40.0 35 {——e1A08 3aet<br>37.5 30 4— 24<br>Coi 35.0 Co22s<br>gg<br>832.5, q 20<br>55<br>230.0 a<br>2 210<br>275 m3W — Opa=9W 5<br>25.0 — yn26W — Oyey=12W<br>° 10 20 30 40 50 ° 10 20 30 40 50<br>time [min] time [min]<br>E. Ta vs ambient (J =2 A) F. Ta vs airflow (J=2 A)<br>35 Tore =20 “C= Tare =30 *C 25.0 — 05xflow = —— 15x flow<br>— Tonw=25 °C — Tago 35°C — nominal<br>30 22.5<br>oCa is)L 20.0<br>g*g<br>28 20 2ei75<br>a a<br>Eas g 150<br>12.5<br>rT)<br>10.0<br>° 10 20 30 40 50 ° 10 20 30 40 50<br>time [min] time [min]<br><!-- End of picture text -->

Figure 3: Open-loop experiments A–F with the placeholder parameter set. 

**A. Consistency test (** _Q_ **gen** = 0 **,** _I_ = 0 **, all states at** _T_ **amb).** All five temperatures remain at 25.000 °C; the maximum drift over 50 min is 0 to machine precision. This verifies that no spurious source or sign error exists in the code, and it is the first test any implementation should pass. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

27 

**B. Sensor array active, no cooling.** Steady state: _Ts_ = 55 _._ 3<sup>_◦_</sup> C, _Ta_ = 36 _._ 1<sup>_◦_</sup> C, _Tw_ = 33 _._ 6<sup>_◦_</sup> C, _Th_ = 26 _._ 7<sup>_◦_</sup> C. The board sits 19 K above the air, the air 11 K above ambient. The hot side is warm even at zero current because heat leaks through the module by conduction, the passive path identified in Figure 2. 

**C. Effect of internal dissipation.** _Ta_ rises linearly with _Q_ gen at 1.24 K/W, which corresponds to an apparent air-to-ambient conductance of 0.808 W/K. The linearity is exact because all passive paths are linear, and it is what makes _Q_ gen identifiable from experiment E2. 

Table 11: Steady-state temperatures versus Peltier current ( _Q_ gen = 9 W<sup>[P]</sup> , _T_ amb = 25<sup>_◦_</sup> C). 

|_I_ (A)|_Ts_|_Ta_|_Tw_|_Tc_|_Th_(°C)|
|---|---|---|---|---|---|
|0|55.3|36.1|33.6|34.3|26.7|
|1|43.5|22.0|26.5|18.7|28.7|
|2|35.2|12.0|21.5|7.7|32.3|
|3|29.9|5.7|18.3|0.8|37.3|
|4|27.1|2.3|16.7|_−_2_._9|43.7|



**D. Effect of Peltier current.** Three features of this table are important. The relationship is clearly _nonlinear_ : the first ampere buys 14 K, the fourth buys 3.4 K, because Joule heating grows as _I_<sup>2</sup> while pumping grows as _I_ . The hot side climbs steadily, degrading the cold side through _K_ ( _Th − Tc_ ). And at 4 A the cold-side assembly is below 0 °C — which risks frost and condensation. Section 21.4 discusses this limit. 

**E, Ambient sensitivity.** A 15 K change in ambient moves the open-loop chamber temperature by 13.8 K, nearly one-for-one. Ambient is therefore the single largest disturbance, and it must be logged with every DGA measurement whether or not the loop is closed. 

**F. Airflow.** Halving the airflow (scaling _Gac_ and _Gh∞_ by 0 _._ 5<sup>0</sup><sup>_._8</sup> ) raises _Ta_ by 4.8 K at _I_ = 2 A; increasing it by 50 % lowers _Ta_ by 2.1 K. A blocked filter or a failing fan is thus a first-order fault. 

### **21.3 Energy-balance verification** 

At _I_ = 2 A the converged steady state gives 

in: _Q_ gen + _P_ elec = 9 _._ 000 W + 10 _._ 205 W = 19 _._ 205 W _,_ 

out: _Gw∞_ ( _Tw − T_ amb) + _Gh∞_ ( _Th − T_ amb) = _−_ 1 _._ 739 W + 20 _._ 944 W = 19 _._ 205 W _,_ 

a residual of 1.1 × 10<sup>−14</sup> W. Two checks pass simultaneously: the code conserves energy, and the thermoelectric identity _Qh − Qc_ = _P_ elec holds (20 _._ 944 _−_ 10 _._ 739 = 10 _._ 205). 

**A sign that is easily missed.** The wall term is _negative_ : at _I_ = 2 A the wall is at 21.5 °C, below the 25 °C ambient, so 1.74 W flows _into_ the chamber from the room. Once the chamber is cooled below ambient, the enclosure stops being a heat sink and becomes a heat source. Any model that hard-codes the wall as a loss term will be wrong in exactly the regime the controller operates in. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

28 

### **21.4 Actuator authority and the admissible setpoint range** 



<!-- Start of picture text -->
Static characteristic Local process gain<br>40 —— sen=42%en=9W W 2—4 |]— Oyen2 en=9 W W<br>o 2% en=14 W 2 en=14W<br>2 30 -6<br>G <<br>a gz -8<br>3 20 =<br>2 3 -10<br>3§= 10 5 “RR<br>.<br>-14<br>° -16<br>00 05 10 15 20 25 30 35 40 00 05 10 15 20 25 30 35 40<br>1A] HAL<br><!-- End of picture text -->

Figure 4: Left: steady-state chamber temperature versus Peltier current for three internal loads; the dotted line is ambient. Right: the local process gain _dTa/dI_ , which varies by roughly a factor of three across the current range. 

With unipolar drive the reachable steady-state set at _Q_ gen = 9 W<sup>[P]</sup> and _T_ amb = 25<sup>_◦_</sup> C is 



but the _usable_ range is narrower and is set by three practical bounds: 

1. **Upper bound:** the passive equilibrium. No setpoint above 36.1 °C is reachable without a heater or bipolar drive. This bound moves with _Q_ gen and with ambient, so a setpoint valid in an air-conditioned laboratory may be unreachable on site. 

2. **Lower bound (dew point).** Cooling the chamber below the dew point of the gas inside it condenses water onto the sensors. MQ/TGS sensors are strongly humidity sensitive and condensation invalidates the measurement outright. The lower setpoint bound is therefore _T_ dew+ margin, not _Ta_ ( _I_ max). 

3. **Cold-plate frost.** At _I_ = 4 A the plate is at −2.9 °C<sup>[P]</sup> . Frost on the cold-side heat exchanger raises _Rac_ over time, which the controller sees as a slow loss of gain. 

**Answer to “what setpoints should be used?”** They cannot be chosen from the thermal model alone. The admissible interval is [ _T_ dew + ∆ _, Ta_ ( _I_ = 0) ], and both ends must be measured on the instrument in its intended environment. The dry-run set _{_ 27 _,_ 30 _,_ 32 _,_ 33 _}_<sup>_◦_</sup> C used below is chosen only to exercise both directions of the loop within the reachable range. 

## **22 System Identification** 

### **22.1 Operating-point dependence** 

Linearising at four currents and evaluating the DC gain _−CA_<sup>_−_1</sup> _B_ from _I_ to _Ta_ : 

Table 12: Process gain versus operating point. 

|_I_0(A)|_Ta,_0(°C)|_K_ =_dTa/dI_ (K/A)|relative to_I_0 = 2 A|
|---|---|---|---|
|0.5|28.5|_−_14_._15|1.76|
|1.0|22.0|_−_11_._91|1.48|
|2.0|12.0|_−_8_._03|1.00|
|3.0|5.7|_−_4_._78|0.60|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

29 

The gain varies by a factor of 2.96 over the range. A fixed-gain controller tuned at 3 A would be roughly three times too aggressive at 0.5 A. 

### **22.2 Step-response identification** 

A step _I_ : 1 _._ 5 _→_ 2 _._ 0 A was applied at the corresponding equilibrium and the _measured_ output _Tm_ recorded. Deliberately, since the controller can only use what it can measure. 



<!-- Start of picture text -->
Open-loop step /:1.5>2.0A Identification residuals<br>° — full model (Ta) 04 — FOPOT (RMSE=0.157 K)<br>=~ FoPoT —<br>2p)iz (RMSE=0.035K)<br>+++ 2pole/l-zero 02<br>-1<br>Zz oo<br>Z-2 3<br>a q 3H<br>§ 4 302<br>3 gz<br>-0.4<br>.<br>“4 N 0.6<br>o 5 W 15 2 2 30 35 40 o 5 1 15 2 2 30 35 40<br>time [min] time [min]<br><!-- End of picture text -->

Figure 5: Open-loop step response and two candidate reduced models, with residuals. 

#### **First-order plus dead time (FOPDT).** 



with fit RMSE 0.157 K. The identified dead time is zero, as it should be: a lumped RC network has no transport delay. The apparent initial sluggishness comes from the measurement lag and the fast poles, not from a delay. 

**Second order with a lead zero.** The residual of the FOPDT fit is systematic: it under-predicts early and over-predicts late. This pattern is the signature of two well-separated modes. Fitting 



gives 



with RMSE 0.035 K, a factor of 4.5 better. 

**Physical reading of the reduced model.** The two poles are the wall (275 s) and the coldplate/board group (35 s). The lead zero at 183 s lies between them and partially cancels the slow pole: physically, the chamber responds quickly through the cold-plate path and then drifts slowly as the wall follows. This is why a single first-order model fits badly, and why the closed loop behaves better than the 275 s pole alone would suggest. 

**Why the identified model changes with setpoint.** Because _K_ depends on the operating point (previous subsection) and because _h_ rad _∝ Tm_<sup>3,theidentificationmustberepeatedateachsetpoint.Thereduced</sup> model above is valid near _I ≈_ 1 _._ 75 A only. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

30 

## **23 PID Design** 

### **23.1 Structure** 

The error is defined as _e_ = _T_ set _− y_ with _y_ = _Tm_ . Because the process gain is _negative_ (more current gives lower temperature), the control law carries an explicit sign inversion: 



Four implementation details are not optional here: 

#### **Derivative on measurement.** 

The derivative acts on _−y_ ˙ rather than on _e_ ˙. With derivative on error, a setpoint step produces an impulse in _u_ ; in this plant, where the air node has a 0.15 s time constant, that impulse drives the cold-side assembly hard and produces a large undershoot. Switching to derivative-on-measurement removed a 3.4 K undershoot in the dry-run. 

#### **Derivative filtering.** 

_Kds D_ ( _s_ ) = 1 + ( _Td/N_ ) _s_<sup>with</sup><sup>_N_= 8.Unfiltered derivative on a quantised SHT20 reading is unusable.</sup> 

#### **Anti-windup.** 

Conditional integration: the integrator is frozen whenever the output is saturated and the error would drive it further into saturation. Essential because the actuator saturates on every large step-down. 

#### **Actuator saturation.** 

0 _≤ u ≤ I_ max. The lower limit at zero is what makes the plant one-directional and is the origin of the asymmetry seen in Section 25. 

### **23.2 Tuning** 

Two candidate tunings were derived from the identified model using the Simple Internal Model Control (SIMC) rules, then screened by simulation on a single 36.1 _→_ 30 °C step [16]. 

Table 13: Candidate tunings and screening result. _J_ = IAE + 20 _·_ OS was used to select, penalising overshoot heavily because overshoot in a cooled chamber risks condensation. 

|Rule|_Kp_|_Ki_|_Kd_|IAE (K s)|overshoot|
|---|---|---|---|---|---|
|SIMC-PI on the zero-cancelled first-order model|0.280|0.00799|0|116.2|40.5 %|
|**SIMC-PID on the second-order model**|**0.503**|**0.00206**|**17.62**|235.2|**2.2 %**|



The second row is selected. Units: _Kp_ in A/K, _Ki_ in A/(K s), _Kd_ in A s/K. Sample time _Ts_ = 1 s, chosen as roughly _τ_ 2 _/_ 35. Fast enough to be transparent to the loop, slow enough for the SHT20 conversion time. 

This baseline exists to be beaten. The comparison in Section 25 is meaningful only because the PID was tuned by a defensible rule on the identified model, not detuned to flatter the fuzzy controller. 

## **24 Fuzzy-PID Design** 

### **24.1 Architecture** 

A gain-adaptation (self-tuning) structure is used rather than a direct fuzzy controller, following the established practice for nonlinear thermal plants [17, 18]: the fuzzy system does not compute _u_ , it computes 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

31 

corrections to the PID gains. 



with ∆ _K• ∈_ [ _−_ 1 _,_ 1] from the fuzzy inference and adaptation depths _ap_ = _ai_ = _ad_ = 0 _._ 7<sup>[P]</sup> . This choice is deliberate: it degrades gracefully. If the fuzzy layer is disabled, the controller reverts exactly to the baseline PID. 

### **24.2 Normalisation** 

The fuzzy inputs are the normalised error and error rate, 



_Ke_ and _K_ ˙ _e_ are not free decoration: they set the error magnitude at which the fuzzy layer becomes fully active. They should be chosen from the process, not guessed: 



the second being the fastest error rate the actuator can produce. The dry-run uses _Ke_ = 2 _._ 5 K and _K_ ˙ _e_ = 0 _._ 08 K _/_ s<sup>[P]</sup> . 

Table 14: Effect of the normalisation factor _Ke_ (dry-run). Smaller _Ke_ makes the fuzzy layer act on smaller errors, improving tracking but slightly degrading disturbance peaks. 

|_Ke_ (K)|IAE, setpoint step (K s)|peak error, load disturbance (K)|IAE, disturbance|
|---|---|---|---|
|1.5|219.4|0.322|270.7|
|2.5|235.1|0.318|268.6|
|4.0|255.3|0.316|266.2|
|6.0|280.7|0.314|263.5|



### **24.3 Membership functions and inference** 

Seven triangular membership functions per input, labelled NB, NM, NS, ZO, PS, PM, PB, with apexes evenly spaced at _{−_ 1 _, −_<sup><u>2</u></sup> 3<sup>_, −_</sup><sup><u>1</u></sup> 3<sup>_,_0</sup><sup>_,_</sup> 3<sup><u>1</u></sup><sup>_,_</sup><sup><u>2</u></sup> 3<sup>_,_1</sup><sup>_}_and50 %overlapbetweenneighbours.Triangularfunctions</sup> with even spacing are chosen because they give a partition of unity (<sup>�</sup> _i_<sup>_µi_= 1 everywhere), which makes</sup> the interpolation between rules linear and predictable, a property that Gaussian functions do not have and that matters when the fuzzy layer must be defended in a review. 

Inference is zero-order Takagi–Sugeno: product _t_ -norm for the rule firing strength and weighted-average defuzzification, 



where _c_<sup>_•_</sup> _ij_<sup>_∈{−_1</sup><sup>_, −_</sup> 3<sup><u>2</u></sup><sup>_, . . . ,_1</sup><sup>_}_is the singleton consequent of rule (</sup><sup>_i, j_).Because the membership functions</sup> form a partition of unity the denominator is unity and the scheme is exactly a smooth interpolation of the rule table. Computationally trivial, which matters for a Teensy 4.1 implementation. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

32 

### **24.4 Rule base and its reasoning** 

Table 15: Rule matrices. Rows: _eN_ from NB to PB. Columns: _e_ ˙ _N_ from NB to PB. Entries are the singleton consequents for ∆ _Kp_ , ∆ _Ki_ and ∆ _Kd_ . 

||||∆|_Kp_|||||||∆|_Ki_|||||||∆|_Kd_||||
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
||NB|NM|NS|ZO|PS|PM|PB||NB|NM|NS|ZO|PS|PM|PB||NB|NM|NS|ZO|PS|PM|PB|
|NB|PB|PB|PM|PM|PS|ZO|ZO|NB|NB|NB|NM|NM|NS|ZO|ZO|NB|PS|NS|NB|NB|NB|NM|PS|
|NM|PB|PB|PM|PS|PS|ZO|NS|NM|NB|NB|NM|NS|NS|ZO|ZO|NM|PS|NS|NB|NM|NM|NS|ZO|
|NS|PM|PM|PM|PS|ZO|NS|NS|NS|NB|NM|NS|NS|ZO|PS|PS|NS|ZO|NS|NM|NM|NS|NS|ZO|
|ZO|PM|PM|PS|ZO|NS|NM|NM|ZO|NM|NM|NS|ZO|PS|PM|PM|ZO|ZO|NS|NS|NS|NS|NS|ZO|
|PS|PS|PS|ZO|NS|NS|NM|NM|PS|NM|NS|ZO|PS|PS|PM|PB|PS|ZO|ZO|ZO|ZO|ZO|ZO|ZO|
|PM|PS|ZO|NS|NM|NM|NM|NB|PM|ZO|ZO|PS|PS|PM|PB|PB|PM|PB|PS|PS|PS|PS|PS|PB|
|PB|ZO|ZO|NM|NM|NM|NB|NB|PB|ZO|ZO|PS|PM|PM|PB|PB|PB|PB|PM|PM|PM|PS|PS|PB|



The rules are not arbitrary; each block encodes a standard control argument. 

#### **Large error, error growing (corners NB/NB and PB/PB).** 

The controller is far from target and moving the wrong way. Raise _Kp_ for speed, cut _Ki_ to prevent the integrator charging while the actuator is saturated (a fuzzy complement to the anti-windup logic), and set _Kd_ high to add damping before the approach. 

#### **Large error, error shrinking.** 

Already converging fast. Reduce _Kp_ to avoid overshoot; _Ki_ may begin to recover. 

#### **Small error, small rate (centre ZO/ZO).** 

Near steady state. Lower _Kp_ , raise _Ki_ to remove residual offset, keep _Kd_ small to avoid amplifying measurement noise. The loop spends most of its life in this region during a DGA measurement, and noise amplification is the main risk there. 

#### **Small error, large rate.** 

A disturbance is arriving. Raise _Kd_ to react before the error grows. 

### **24.5 Known limitation of this rule base** 

The matrices are symmetric in the sign of the error, which implicitly assumes the actuator is equally capable in both directions. It is not: with unipolar drive the controller can cool but not heat. Section 25 shows the consequence, on the heating transition the fuzzy layer raises the gains in a direction where the actuator is already at its lower limit, producing overshoot that the fixed-gain PID avoids. Two remedies are available and both are recommended for the next iteration: make the rule base asymmetric in _e_ , or provide bipolar drive so that the symmetry assumption becomes true. 

## **25 Multi-Setpoint Simulation** 

The setpoint profile 36 _._ 1 _→_ 33 _→_ 30 _→_ 27 _→_ 32<sup>_◦_</sup> C exercises a step-down sequence of decreasing reachability followed by a step-up that the unipolar actuator can only achieve passively. 

### **25.1 Performance metrics** 

For a segment of duration _T_ with error _e_ ( _t_ ) = _T_ set _− Ta_ ( _t_ ) and actuator current _u_ ( _t_ ): 



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

33 

with _P_ elec = _αI_ ( _Th − Tc_ ) + _I_<sup>2</sup> _Re_ from Section 13. ITAE is included because it penalises late error and therefore separates controllers that settle from controllers that merely start quickly, the distinction that matters for a chamber that must be thermally quiet before a DGA measurement begins. Steady-state ripple is reported as the peak-to-peak excursion of _Ta_ over the final 300 s of each segment. 



<!-- Start of picture text -->
Multi-setpoint tracking (dry-run parameter set)<br>36 ===. setpoint<br>— PD<br>34 — fuzzy-PID<br>2 32 ry<br>* 30 Ls H<br>28<br>is<br>° 10 20 30 40 50 60<br>Zoom: 33 + 30 °C transition<br>3<br>— PD<br>— fuzzy-PiD<br>32<br>o<br>Kost<br>30 [----------------- ----<br>0 100 200 300 400 500<br>time after step [s]<br>25 — PD<br>— fuzzy-PiD<br>2.0<br>gis<br>10<br>05<br>0.0<br>° 10 20 30 40 50 60<br>150 — kylKon<br>< — KiKio<br>8 125 — Katka<br>3<br>8 1.00<br>E 0.75<br>2 0.50<br>0.25<br>° 10 20 30 40 50 60<br>time [min]<br><!-- End of picture text -->

Figure 6: Multi-setpoint tracking. Top: chamber temperature. Second: zoom on the 33 _→_ 30<sup>_◦_</sup> C transition. Third: Peltier current. Bottom: the normalised gains produced by the fuzzy layer. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

34 

Table 16: Multi-setpoint performance (dry-run parameter set). _tr_ = 10 % to 90 % rise time, _ts_ = 2 % settling time, OS = overshoot as a percentage of the step; IAE, ISE and ITAE over the 900 s segment; effort = � _u_<sup>2</sup> _dt_ ; _u_ ¯ = mean current; _E_ = Peltier electrical energy; ripple = peak-to-peak _Ta_ over the last 300 s. 

|Transition<br>(°C)|Ctrl.|_tr_<br>(s)|_ts_<br>(s)|OS<br>(%)|IAE|ISE|ITAE<br>(_×_10<sup>3</sup>)|¯_u_<br>(A)|_E_<br>(W h)|ripple<br>(K)|
|---|---|---|---|---|---|---|---|---|---|---|
|36_._1_→_33|PID|81.7|444|2.23|113.2|81.8|18.5|0.221|0.015|0.028|
||F-PID|**65.0**|**135 **|**1.35**|**91.1**|**73.8**|**12.6**|0.221|0.017|**0.019**|
|33_→_30|PID|83.1|457|2.25|111.7|80.5|18.2|0.419|0.086|0.027|
||F-PID|**66.3**|**144 **|**1.25**|**87.4**|**67.8**|**11.8**|0.419|0.087|**0.017**|
|30_→_27|PID|86.5|406|2.03|112.7|83.5|17.6|0.632|0.223|0.026|
||F-PID|**69.4**|**163 **|**1.05**|**88.7**|**69.3**|**11.2**|0.632|0.224|**0.015**|
|27_→_32|PID|**99.3**|**183 **|**1.26**|**212.1**|**359.9**|**24.2**|0.225|0.017|**0.030**|
|(passive)|F-PID|106.0|653|4.56|330.9|562.9|55.5|0.222|0.017|0.075|



### **25.2 Reading the results** 

**Cooling transitions.** The fuzzy layer improves every metric: IAE by 19 % to 22 %, rise time by 18 % to 20 %, overshoot by about 45 %, and settling time by a factor of 2.5 to 3.4. The control effort is essentially unchanged (within 5 %) and the Peltier energy consumed per transition is identical to within 2 %, so the improvement is not bought with actuator wear or with power. ITAE, which weights late error, improves by 32 % to 36 % — a larger margin than IAE, confirming that the gain is in settling rather than in initial speed. Steady-state ripple also falls from about 0.027 K to 0.017 K. The mechanism is visible in the bottom panel of Figure 6: at each step _Kp_ jumps to 1.47 times its nominal value for a few seconds and _Kd_ falls to 0.3, then both relax as the error closes. 

**The heating transition is worse, and this matters.** On 27 _→_ 32<sup>_◦_</sup> C the actuator can only release cooling and wait; the plant heats at its own passive rate. The fuzzy layer, seeing a large error, raises _Kp_ and _Ki_ , which does nothing to speed up the approach but does charge the integrator, producing 4.6 % overshoot against the PID’s 1.3 % and a settling time 3.6 times longer. 

**Assessment of the fuzzy layer.** Fuzzy gain adaptation is useful where the actuator has authority and the plant gain varies. That is the cooling direction, where it gives a 20 % improvement in Integral Absolute Error (IAE) at equal control effort. It is counterproductive in the direction where the actuator is saturated at zero. Reporting only the favourable transitions would be misleading. The fix is structural (asymmetric rules or bipolar drive), not a matter of retuning. 

## **26 Disturbance Simulation** 

Two disturbances were applied at a fixed setpoint of 30 °C: a 4 W step in internal dissipation (a plausible consequence of the heater array switching to a higher duty cycle) from 10 min to 25 min, and a 6 K ambient step from 35 min to 50 min. 

35 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 



<!-- Start of picture text -->
Disturbance rejection: +4 W internal (10-25 min); +6 K ambient (35-50 min)<br>36 ===" setpoint<br>— PD<br>35 — fuzzy-PiD<br>34<br>2 33<br>© 32<br>31<br>30 ann --- Senna nnn neonSSS oon nnnnn nnn nn= ee naan:<br>4 — PD<br>— fuzzy-PiD<br>3<br>£2<br>1<br>°<br>° 10 20 30 40 50 60<br>time [min]<br><!-- End of picture text -->

Figure 7: Disturbance rejection. Shaded bands mark the disturbance intervals. 

Table 17: Disturbance rejection (dry-run). 

|||+4 Wint|ernal||+6 Kam|bient|
|---|---|---|---|---|---|---|
|Controller|peak (K)|IAE|ITAE (_×_10<sup>3</sup>)|peak (K)|IAE|ITAE (_×_10<sup>3</sup>)|
|PID|0.315|267.6|205.6|0.426|357.2|285.2|
|Fuzzy-PID|0.318|268.6|201.0|0.443|355.1|268.9|



Both controllers hold the chamber to better than 0.5 K against disturbances that would move the open-loop temperature by 6.2 K and 5.5 K respectively, a rejection ratio of roughly 13:1. 

The two controllers are, however, indistinguishable here, and the reason is instructive rather than disappointing: the peak error never exceeds 0.45 K, which is 18 % of _Ke_ = 2 _._ 5 K, so the fuzzy inputs stay near the ZO/ZO cell and the gain corrections are small. An error-normalised adaptation scheme is by construction a large-signal mechanism. Comparable findings are reported elsewhere: in an embedded Peltier regulation study, Fuzzy-PID was outperformed in ITAE by a supervised fractional-order scheme [18], and an adaptive neural scheme outperformed a fuzzy controller on a furnace plant [19]. Fuzzy gain adaptation is a useful tool, not a guaranteed improvement, and the claim must be tested per plant. If improved small-signal disturbance rejection is wanted, the correct move is to reduce _Ke_ (Section 24.2 quantifies the trade-off) or to add feedforward from the measured _Q_ gen and _T_ amb, both of which are known quantities in this instrument, which makes feedforward the more promising route. 

## **27 Sensitivity Analysis** 

### **27.1 Steady-state sensitivity** 

Each parameter was perturbed by _±_ 20 % and the change in steady-state _Ta_ recorded at _I_ = 2 A, _Q_ gen = 9 W, _T_ amb = 25<sup>_◦_</sup> C. 

36 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 



<!-- Start of picture text -->
Parameter sensitivity (/ = 2 A, Qgen = 9 W, Tamp = 25 °C)<br>alpha<br>T_amb<br>Kp<br>Qgen<br>Gho<br>Re_p<br>Gac<br>Gaw<br>Gsa<br>G.wo<br>cw<br>ch<br>cs<br>cace9) | mEmm 20%+20%<br>-4 -2 0 2 4<br>change in steady-state Ta [K]<br><!-- End of picture text -->

Figure 8: Steady-state parameter sensitivity, sorted by magnitude. 

Table 18: Ranked steady-state sensitivity (change in _Ta_ , K). 

|Rank|Parameter|_−_20 %|+20 %|Interpretation|
|---|---|---|---|---|
|1|_α_|+5_._30|_−_5_._03|Seebeck coefficient sets pumping directly|
|2|_T_amb|_−_4_._53|+4_._53|ambient passes through almost 1:1|
|3|_K_|_−_3_._12|+2_._37|back conduction opposes cooling|
|4|_Q_gen|_−_2_._02|+2_._02|internal load|
|5|_Gh∞_|+1_._03|_−_0_._70|heatsink quality, via_Th_|
|6|_Re_|_−_0_._97|+0_._97|Joule heating|
|7|_Gac_|+0_._70|_−_0_._48|cold-side coupling|
|8|_Gaw_|_−_0_._57|+0_._46|internal surface exchange|
|9|_Gsa_|_−_0_._36|+0_._27|board-to-air coupling|
|10|_Gw∞_|_−_0_._27|+0_._21|external losses|
|11–15|all capacitances|_≈_0|_≈_0|by construction: capacitances do not enter the steady state|



### **27.2 Dynamic sensitivity** 

A steady-state analysis is blind to the capacitances, so the same perturbation study was repeated on the dominant closed-form time constant ( _τ_ dom = 253 _._ 2 s at the nominal point). 

Table 19: Ranked sensitivity of the dominant time constant (change in _τ_ dom, s). 

|Rank|Parameter|_−_20 %|+20 %|Interpretation|
|---|---|---|---|---|
|1|_Cw_|_−_33_._3|+35_._6|wall mass dominates the slow mode|
|2|_Gw∞_|+34_._1|_−_25_._6|the wall’s only escape path|
|3|_Cs_|_−_12_._7|+14_._8|board mass|
|4|_K_|+8_._8|_−_6_._6|couples the slow mode to the actuator|
|5|_Gsa_|+6_._1|_−_4_._2||
|6–9|_Gh∞, Gaw, Gac, α_|_<_3|_<_3||
|10|_Cc_|_−_1_._9|+1_._9||
|11|_Ch_|_−_0_._3|+0_._3||
|12|_Ca_|_−_**0**_._**05**|+**0**_._**05**|**chamber air volume is dynamically irrelevant**|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

37 

**Structural result 4, where to spend measurement effort.** Combining both tables, the parameters that must be characterised accurately are, in order: the Peltier triplet ( _α, K, Re_ ); the ambient temperature (which must be logged, not assumed); _Q_ gen; the wall capacitance _Cw_ and the wall-toambient conductance _Gw∞_ ; and the board capacitance _Cs_ . 

The free-air volume _V_ air, the quantity that consumed Sections 2 and 3. Affects the dominant time constant by 0.05 s out of 253 s, i.e. 0.02 %. Determining it to better than 20 % accuracy has no measurable benefit for the thermal model. It remains worth knowing for gas-exchange and residence-time calculations in the DGA application, which is a different question from the thermal one. 

## **28 Uncertainty Analysis** 

### **28.1 Sources and expected magnitudes** 

Table 20: Uncertainty inventory. The propagated effect column uses the sensitivity coefficients of Section 27. 

|Source|Expected spread|Propagated effect on<br>_Ta_|Mitigation|
|---|---|---|---|
|Peltier _α_, _K_, _Re_ from<br>datasheet maxima|10 % to 20 %|3 K to 6 K|identify in situ (E4); this is the domi-<br>nant uncertainty|
|_Q_gen<br>from<br>itemised<br>datasheets|20 % to 40 %|2 K to 4 K|single in-line power measurement (E1)|
|Ambient temperature drift|2 K to 10 K|2 K to 10 K open-<br>loop,_<_0_._5 Kclosed-<br>loop|close the loop; log_T_amb|
|Convection<br>coefficient<br>from correlations|factor 1.5 to 2|0.5 K to 1.5 K|identify_hA_as a lumped product|
|Chamber volume estimate|10 % to 20 %|_<_0_._01 K|none needed|
|Material properties (_cp_,_k_)|5 % to 15 %|0.5 K<br>to<br>2 K<br>(dy-<br>namic)|weigh parts, state materials|
|Fan airflow|20 % to 50 %|1 K to 5 K|measure once; monitor fan tacho in ser-<br>vice|
|Sensor placement / gradi-<br>ents|—|1 K to 20 K between<br>_Ts_ and_Ta_|report the gradient; consider a second<br>SHT20|
|Component<br>tolerance<br>(heaters)|5 % to 10 %|0.3 K to 0.6 K|accept|



### **28.2 Recommended propagation method** 

For a first pass, linear propagation using the sensitivity coefficients already computed is sufficient: 



This is valid because the perturbation study showed near-symmetric responses for most parameters. 

Where it is not valid, specifically for _α_ and _K_ , whose responses to _±_ 20 % differ by 5 % to 30 % in magnitude, and near the actuator saturation limits, a Monte Carlo study is warranted: sample the parameter vector from the distributions above (2000 draws is ample for a six-state model), propagate through the steady-state solver, and report the 5 % to 95 % interval of _Ta_ and of _τ_ dom. The computational cost is negligible; the Newton solver converges in milliseconds. 

A Monte Carlo study should also be run _through the closed loop_ , because the quantity that actually matters is not the uncertainty in the plant but the uncertainty in the controlled temperature, and feedback 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

38 

attenuates the former by roughly the loop gain. A plant parameter with a 5 K open-loop influence may have only a 0.05 K closed-loop influence. Knowing which parameters fall into that category prevents unnecessary measurement work. 

## **29 MATLAB / Python Implementation Procedure** 

### **29.1 Numerical integration** 

The model is written as _x_ ˙ = _f_ ( _x, u, d_ ) and integrated by classical fourth-order Runge–Kutta with a fixed step _h_ : 



The forward-Euler form required by the specification, 



is retained only for the embedded implementation, where it is the natural choice because the controller already runs on a fixed tick. 

### **29.2 Choosing the step size** 

Three different time steps appear in this work and must not be confused: 

|Step|Value|Constraint that sets it|
|---|---|---|
|Integration step_h_|0.1 s|Stability of the explicit solver on the fastest mode: _h <_ 2_._78_τa_ =<br>0_._41 s for RK4. Verified: _h_ = 0_._5 s diverges; _h ≤_0_._4 s agrees to<br>1 × 10<sup>−14</sup>.|
|Control sample time<br>_Ts_|1 s|Fast relative to _τ_2 = 35 s (ratio 35), slow enough for the SHT20<br>conversion and for the I<sup>2</sup>C schedule.|
|Data-logging interval|1 s to 5 s|Adequate to resolve_τ_dom = 253 s; anything faster merely inflates the<br>log.|



**Stiffness is a modelling choice, not a fact.** The 0.15 s air node forces a step 2500 times smaller than the dominant time constant. Two legitimate escapes exist. (i) Use a stiff implicit solver (ode15s, solve_ivp(method=’BDF’)), which removes the stability constraint entirely. (ii) Apply a quasi-steady reduction: set _Ca T_<sup>˙</sup> _a_ = 0 and solve algebraically, 



which is exact to within 0.1 % on any time scale longer than a second and reduces the model to four states. The information lost is only the first fraction of a second of the air response, which no SHT20 could resolve anyway. Route (ii) is recommended for the embedded implementation; route (i) for offline studies where _Ta_ transients matter. 

### **29.3 Code architecture** 

The implementation is organised into single-responsibility modules so that a change in one physical assumption touches exactly one file. Only NumPy, SciPy, Matplotlib and Pandas are used; no machinelearning library is required, and none is used anywhere in the methodology. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

39 

Table 21: Module responsibilities. 

|Module|Responsibility|
|---|---|
|geometry.py|Dimensions;_V_cyl under G1 and G2,_V_box,_V_geom; areas. Returns both interpretations,<br>never one.|
|thermal_parameters|.py<br>Air properties;_Ci_ from masses;_Gij_ from_hA_and_L/kA_; a single dictionary that is<br>the only place a number is written.|
|heat_generation.py|_Q_gen(_t_) from measured heater power and the firmware duty schedule; constant,<br>pulsed or measured waveform.|
|peltier_model.py|Eqs.(33)–(34); datasheet-to-parameter conversion; the identity check_Qh −Qc_ =<br>_P_elec.|
|thermal_model.py|_f_(_x, u, d_) and the analytic or numerical Jacobian. The physics lives here and<br>nowhere else.|
|simulation.py<br>system_identificat<br>pid_controller.py|RK4 /solve_ivpdrivers, Newton steady state, time-step checks.<br>ion.py<br>Step generation, FOPDT and two-pole/one-zero fits, residual reporting.<br> Discrete PID with derivative-on-measurement, filtering, saturation and anti-windup.|
|fuzzy_pid.py|Membership functions, the three rule matrices, inference and defuzzification; reduces<br>topid_controllerwhen disabled.|
|experiments.py|The open-loop experiments A–F, the multi-setpoint profile, the disturbance scenar-<br>ios.|
|validation.py|RMSE, MAE,_e_max; simulation-versus-measurement comparison at each operating<br>point.|
|plotting.py|All figures, one function per figure, publication settings in one place.|



### **29.4 Symbolic, discrete and code form of every state equation** 

Each state equation is written three times: continuous, discrete (forward Euler shown, since it is the form the embedded controller uses), and as the Python expression. 

Table 22: Mapping from physics to code. dt is the integration step, P the parameter dictionary. 

|State|Continuous / discrete|Python|
|---|---|---|
|_Ts_|_Cs_ <sup>˙</sup>_Ts_<br>=<br>_Q_gen _−Gsa_(_Ts−Ta_) _−_<br>_Gsw_(_Ts−Tw_)<br>_Ts_[_k_+1] =_Ts_[_k_] + <sup>∆</sup><sup>_t_</sup><br>_Cs_<br>�<br>_·_<br>�|dTs = (Qgen - P["G_sa"]*(Ts-Ta) -<br>P["G_sw"]*(Ts-Tw))/P["C_s"]<br>Ts_next = Ts + dt*dTs|
|_Ta_|_Ca_ <sup>˙</sup>_Ta_ =_Gsa_(_Ts−Ta_)_−Gaw_(_Ta−Tw_)_−_<br>_Gac_(_Ta−Tc_)<br>_Ta_[_k_+1] =_Ta_[_k_] + <sup>∆</sup><sup>_t_</sup><br>_Ca_<br>�<br>_·_<br>�|dTa = (P["G_sa"]*(Ts-Ta) - P["G_aw"]*(Ta-Tw) -<br>P["G_ac"]*(Ta-Tc))/P["C_a"]<br>Ta_next = Ta + dt*dTa|
|_Tw_|_Cw_ <sup>˙</sup>_Tw_<br>=<br>_Gaw_(_Ta−Tw_)<br>+<br>_Gsw_(_Ts−Tw_)_−Gw∞_(_Tw−T_amb)|dTw = (P["G_aw"]*(Ta-Tw) + P["G_sw"]*(Ts-Tw) -<br>P["G_wo"]*(Tw-Tamb))/P["C_w"]|
|_Tc_|_Cc_ <sup>˙</sup>_Tc_ =_Gac_(_Ta−Tc_)_−Qc_(_I, Tc, Th_)|Qc = P["alpha"]*I*(Tc+273.15) - 0.5*I**2*P["Re_p"] -<br>P["K_p"]*(Th-Tc)<br>dTc = (P["G_ac"]*(Ta-Tc) - Qc)/P["C_c"]|
|_Th_|_Ch_ <sup>˙</sup>_Th_<br>=<br>_Qh_(_I, Tc, Th_)<br>_−_<br>_Gh∞_(_Th−T_amb)|Qh = P["alpha"]*I*(Th+273.15) + 0.5*I**2*P["Re_p"] -<br>P["K_p"]*(Th-Tc)<br>dTh = (Qh - P["G_ho"]*(Th-Tamb))/P["C_h"]|
|_Tm_|_τm_ <sup>˙</sup>_Tm_ =_Ta −Tm_,<br>_y_ =_Tm_|dTm = (Ta - Tm)/P["tau_m"]|



**A common implementation error.** The Peltier equations require _absolute_ temperature, the rest of the model is conventionally written in degrees Celsius, and the two are mixed in the same function. Forgetting the +273.15 inside Qc and Qh does not produce an obvious error. The simulation still runs and still looks plausible. It simply scales the pumping term by roughly a factor of ten at room temperature. The energy-balance test of Section 21 catches it; visual inspection does not. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

40 

### **29.5 Staged implementation plan** 

Table 23: Implementation stages, each with its own pass criterion. No stage should be started before the previous one passes. 

|#|Stage|Pass criterion|
|---|---|---|
|1|Geometry and constants|_V_cyl,_V_box reproduce Section 3 by hand calculation|
|2|Volumes|G1 and G2 both computed; the difference reported, not<br>hidden|
|3|Air mass and_Ca_|_Ca_within 1 % of 0.472 J/K at 397 mL|
|4|All thermal capacitances|units checked: every_C_ in J/K, every_G_in W/K|
|5|Thermal resistances|_G_passcomputed both from the network reduction and from a<br>simulated_Q_gen_/_∆_T∞_; the two must agree|
|6|Heat-generation model|_Q_gen = 0_⇒_all states remain at_T_amb (Test A)|
|7|Peltier model|_Qh −Qc_ = _αI_∆_T_ +_I_<sup>2</sup>_Re_ to machine precision;∆_T_max<br>and_I_optplausible|
|8|Coupled ODE model|energy balance closes: _Q_gen + _P_elec = heat rejected, to<br><1 × 10<sup>−9 </sup>W|
|9|Open-loop simulation|Experiments A–F reproduce the qualitative behaviour of<br>Section 21|
|10|Sensitivity study|both the steady-state and the dynamic ranking produced|
|11|Reduced-order identification|residual RMSE reported for every candidate structure, not<br>just the chosen one|
|12|PID design|tuning rule named; gains reproducible from the identified<br>model|
|13|Fuzzy-PID|with the fuzzy layer disabled, output is bit-identical to stage<br>12|
|14|Multi-setpoint simulation|metrics tabulated for every transition,<br>including un-<br>favourable ones|
|15|Disturbance simulation|disturbance magnitudes justified against measured variation|
|16|PID vs Fuzzy-PID comparison|same baseline gains, same disturbances, same metrics|
|17|Comparison with experiment|Section 30|



### **29.6 Notes for the embedded implementation** 

The controller will ultimately run on the Teensy 4.1 alongside the sensing firmware. Four points follow from the analysis above: 

1. The fuzzy inference reduces to two seven-element membership evaluations and a 7 _×_ 7 weighted sum per gain. With a partition of unity, at most four rules fire at once, so the exact computation costs about 12 multiply–accumulates per gain. There is no reason to approximate it with a lookup table. 

2. The actuator is a current, but the driver is almost certainly PWM. The mapping from duty cycle to _effective_ current is nonlinear for a Peltier module because Joule heating depends on _I_<sup>2</sup> while pumping depends on the mean of _I_ : PWM at a duty _D_ gives mean pumping _∝ DI_ but mean Joule heating _∝ DI_<sup>2</sup> , which is worse than DC at the same average current. Either use a linear current source, or PWM fast enough that the module sees a smoothed current, or include the duty-cycle correction explicitly in the model. 

3. Integrator state must be stored in a wide type and reset on setpoint reconfiguration. 

4. Log _u_ , _e_ , all measured temperatures and the ambient temperature at every control tick. The validation in Section 30 is impossible without them. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

41 

## **30 Experimental Validation Plan** 

### **30.1 Principle** 

A simulation is not evidence. The model becomes usable only after it has predicted measurements it was not fitted to. The plan below therefore separates _identification data_ (used to fit parameters) from _validation data_ (used only to test). 

### **30.2 Metrics** 





Transient agreement is additionally assessed by comparing the identified _τ_ dom of simulation and measurement. 

### **30.3 Validation matrix** 

Table 24: Validation runs. Runs V1–V4 are identification; V5–V10 are held out and used only for testing. 

|Run|Condition|Use|What it tests|
|---|---|---|---|
|V1|Passive warm-up,_I_ = 0,_T_amb<br>nominal|fit|_G_pass,_C_eff|
|V2|Current sweep, steady states|fit|_α, Re, K, Gac, Gh∞_|
|V3|Current steps at two operating<br>points|fit|_Cc_,_Ch_, dynamics|
|V4|Heater on/off step|fit|_Gsa_,_Gsw_,_Cs_|
|V5|Current step at a**third**operat-<br>ing point|test|whether the nonlinearity is captured, not<br>merely interpolated|
|V6|Different ambient (±8 K)|test|the_T_amb pathway, the largest disturbance|
|V7|Reduced fan speed|test|the airflow model of Section 12|
|V8|Different heater duty cycle|test|_Q_gen scaling|
|V9|Closed-loop multi-setpoint run|test|the complete loop against the simulated closed<br>loop|
|V10|24 h run under laboratory am-<br>bient drift|test|long-term behaviour, frost accumulation, inte-<br>grator health|



### **30.4 Acceptance criteria** 

These should be fixed _before_ the data are taken, so that the model is not tuned to whatever it happens to achieve: 

- Steady state: _ess ≤_ 1 _._ 0 K on every validation run. 

- Transient: RMSE _≤_ 1 _._ 5 K and _e_ max _≤_ 3 K over the full transient. 

- Time constant: identified _τ_ dom within 25 %. 

- Closed loop (V9): simulated and measured settling times within 30 %; steady-state error within 0.2 K. If the model fails on V5 but passes V1–V4, the nonlinearity is mis-modelled — most likely the Peltier 

- parameters. If it fails on V6, the wall pathway is wrong. If it fails only on V10, the likely culprits are frost on the cold-side heat exchanger or thermal-interface degradation, neither of which the model contains. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

42 

### **30.5 Instrumentation required** 

The two SHT20 units already fitted are not sufficient for validation. At minimum add: a thermocouple or thermistor on the chamber wall, one on the cold-side assembly, one on the hot-side heatsink, and one measuring ambient away from the instrument’s own exhaust. Without _Tw_ , _Tc_ and _Th_ the five-node model cannot be validated node by node, only its output can be checked, which does not distinguish a correct model from a wrong model with compensating errors. 

## **31 Research Decision Gates** 

Table 25: Decision gates. “Status” reflects what this document has established; **OPEN** means a measurement or a decision is still required. 

|#|Question|Status|Basis / what closes it|
|---|---|---|---|
|1|Is the chamber geometry suffi-<br>ciently defined?|**OPEN**|The 34 mm bore has four possible meanings<br>(Sec. 2.2). Closed by inspecting the CAD assem-<br>bly. The_volume_consequence is minor; the_airflow_<br>consequence is not.|
|2|Is<br>the<br>effective<br>air<br>volume<br>known?|**OPEN**,<br>low<br>priority|Bounded to397 mL to 446 mL. Sec. 27.2 shows<br>the thermal model does not care; close it for gas-<br>residence purposes instead.|
|3|Is heat generation known?|**OPEN**, high<br>priority|No value assigned. Closed by one in-line power<br>measurement (E1).|
|4|Is a lumped model valid?|PASS, condi-<br>tional|Bi _≪_1for the metal wall. Weaker for the PCB;<br>verify with a second board-mounted sensor.|
|5|Is forced convection dominant?|**NO**|_h_nc = 5_._1vs_h_fc = 6_._7W/(m<sup>2 </sup>K): mixed convec-<br>tion. Use a combined identified_hA_.|
|6|Is radiation negligible?|**NO**,<br>unless<br>_ε <_0_._1|_h_rad = 5_._7W/(m<sup>2 </sup>K)at_ε_= 0_._9(Sec. 11). Closed<br>by specifying the internal surface finish.|
|7|Is a three-node model sufficient?|Likely,<br>for<br>control|The air node is algebraically slaved; the reduced<br>model needs_Ts_,_Tw_ and the cold-side path.|
|8|Is a five-node model necessary?|YES, for the<br>physics|_Tc_and_Th_are needed because the Peltier equations<br>depend on both, and _Th_ feeds back into cooling<br>capacity.|
|9|Can the Peltier parameters be ob-<br>tained?|YES|Four independent routes (Sec. 13); route 4 recom-<br>mended. This is the highest-sensitivity parameter<br>group.|
|10|Does a linear model represent the<br>plant near each operating point?|YES<br>locally,<br>NO globally|Gain varies by a factor of2.96across the current<br>range (Sec. 22); linearise at each setpoint.|
|11|Does Fuzzy-PID measurably im-<br>prove on PID?|**PARTIALLY**|19 % to 22 %IAE improvement on cooling transi-<br>tions at equal effort;_worse_on the passive heating<br>transition; neutral for small disturbances (Sec. 25).|
|12|Is the actuator authority adequate<br>for the intended setpoints?|**OPEN**|Requires the dew point of the sample gas and the<br>passive equilibrium at the real_Q_gen (Sec. 21.4).|



Gates 11 and 12 are the two that could change the direction of the work. If Gate 12 shows the reachable range does not cover the setpoints the DGA method needs, the answer is a hardware change (bipolar drive, or a heater), and no amount of control design will substitute for it. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

43 

## **32 Final Mathematical Model** 

### **32.1 Consolidated statement** 











with _Tc_ , _Th_ in kelvin inside the bracketed Peltier terms and all conductances _G_ = 1 _/R_ th in W/K. Every symbol appears in the table of Section 19 with its unit, status and method of determination. 

### **32.2 Numerical implementation form** 



with the control law updated every _Ts_ = 1 s: 



### **32.3 The five model levels, and why they differ** 

|Level|Content|Why it is not the same as the next one|
|---|---|---|
|A. Symbolic|Equations above with letters<br>only|Valid for any instrument of this topology; contains no<br>information about_this_chamber.|
|B. Parameterised|Symbols replaced by mea-<br>sured values with uncertainties|Adds instrument-specific knowledge, and with it uncer-<br>tainty. Requires Sec. 18 experiments.|
|C. Numerical|Single point values substituted|Discards the uncertainty of level B. Convenient, but a<br>level-C model quoted without its level-B uncertainty is<br>a misleading object.|
|D. Simulation|Discretised, solver and step<br>chosen|Adds numerical error and a stability constraint that has<br>nothing to do with physics (Sec. 29).|
|E. Control|Reduced order, single operat-<br>ing point|Deliberately_wrong_: it discards the wall pole partly, the<br>air node entirely, and is valid only near one setpoint.<br>Its purpose is tuning, not prediction.|



Confusing level E with level C is the most common failure mode in this kind of work: a controller is designed on a first-order approximation, the approximation is then reported as “the thermal model of the chamber”, and the physics is lost. The ordering used in this document is intended to prevent that. 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

44 

## **33 Recommended Next Experiments** 

In priority order, with the reasoning attached to each: 

1. **Measure** _Q_ **gen (half a day).** One in-line power measurement of everything inside the boundary, over one complete sensing cycle, reporting mean and peak. Sensitivity rank 4 for steady state, and it is the input to almost every other calculation. Also determines whether the heater modulation is fast enough to be filtered by the thermal mass. 

2. **Passive warm-up and cool-down with no cooling (half a day).** Gives _G_ pass from _Q_ gen _/_ ∆ _T∞_ and _τ_ dom from the exponential fit, hence _C_ eff. Two numbers that between them fix most of the open-loop behaviour, obtained with a thermometer and a stopwatch. 

3. **Obtain the Peltier datasheet and extract** ( _α, Re, K_ ) **(one hour).** Highest sensitivity of any parameter group. Even datasheet-derived values at 10 % to 20 % accuracy would remove the largest single uncertainty in this document. 

4. **Resolve the geometry (one hour, CAD).** Settles G1 versus G2 and, more importantly, establishes the internal wetted area _A_ int and the airflow path. Neither of which can be inferred from the dimension list. 

5. **Instrument** _Tw_ **,** _Tc_ **,** _Th_ **and ambient (one day).** Without these the five-node model can never be validated node by node. This is the prerequisite for every subsequent claim about the model’s correctness. 

6. **Current sweep to steady state (one day).** Produces the static characteristic of Figure 4 for the real instrument, from which the reachable temperature range, the gain variation and the identifiability of the Peltier parameters all follow directly. 

7. **Measure the dew point of the sample gas and fix the lower setpoint bound.** A control specification that ignores condensation will eventually destroy a measurement, and possibly a sensor array. 

8. **Determine** _τm_ **for the SHT20 in situ.** It is the only phase lag in the loop and therefore the limit on achievable bandwidth. A step between two stirred baths, compared against a fine thermocouple, is sufficient. 

9. **Then, and only then, repeat this entire document with measured parameters.** Every number in Sections 21–28 should be regenerated. The code is written so that this requires editing one dictionary. 

10. **Decide on bipolar drive.** Gate 12. If setpoints above the passive equilibrium are needed, which is likely if the DGA method calls for elevated sensor baselines, the unipolar actuator is a hard limitation, and the asymmetry it creates is also the one place where the fuzzy layer currently underperforms. 

**What this document has and has not established.** It has established the model structure, the sign conventions, the identifiability of each parameter, the numerical constraints, the reduction path from five nodes to a control model, a defensible PID baseline, a fuzzy architecture with justified rules, and an honest account of where that architecture helps and where it does not. It has established no property of the physical instrument, because no parameter of the physical instrument has yet been measured. The distance between these two statements is exactly the list above. 

## **34 Research Contribution** 

The contributions below are stated as claims that this work makes plausible, not as claims that it has proved. Each is paired with the evidence that would be needed to establish it. 

45 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

Table 26: Candidate contributions and their evidential status. 

|#|Claim|Status here|What would establish it|
|---|---|---|---|
|1|A physically derived multi-node thermal<br>model of an E-Nose chamber whose dom-<br>inant load is its own MQ/TGS sensor array|derived, not calibrated|experiments E1–E8 and valida-<br>tion runs V1–V10|
|2|Quantitative demonstration that the enclosed<br>air volume is thermally negligible while the<br>wall and board masses dominate|demonstrated_within the_<br>_model_, with an explicit<br>sensitivity ranking|repeat on the calibrated model;<br>the conclusion is structural and is<br>unlikely to reverse|
|3|Identification of radiation as a non-negligible<br>internal transfer mode at high surface emis-<br>sivity in a small, low-velocity chamber|quantified as an order-of-<br>magnitude comparison|measurement of the internal sur-<br>face finish|
|4|A control-oriented reduced model (two poles,<br>one lead zero) extracted _from_ the physical<br>model rather than postulated|obtained with RMSE<br>0.035 K against the full<br>model|the same procedure applied to<br>measured step data|
|5|Fuzzy-PID gain adaptation evaluated against<br>a defensibly tuned PID baseline across mul-<br>tiple setpoints, including the unfavourable<br>direction|simulated on the dry-run<br>parameter set|repetition on the calibrated model<br>and on hardware|



Existing E-Nose work on transformer DGA has concentrated on the sensing and classification problem: sensor selection and machine-learning interpretation of MQ/TGS arrays [3], regression of dissolved-gas concentrations from MOS arrays [4], and characterisation of dissolved gases in dielectric oils [5], against the background of the conventional DGA interpretation methods [20]. The thermal environment in which those arrays operate is usually treated as a fixed condition rather than as a designed one, even though heatersupply and ambient effects on metal-oxide response are documented [1, 2]. Treating the chamber thermally, deriving the plant, then controlling it, is the gap this document addresses. Whether that constitutes novelty in the publication sense should be settled by a systematic search, not asserted here. 

## **35 Recommended Experimental Parameters to Be Measured Before Numerical Calibration** 

This section is the practical output of the whole document. The ordering is by expected reduction in model uncertainty per unit of measurement effort, using the sensitivity rankings of Sections 27–27.2. 

46 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

Table 27: Measurement priority list. No numerical values are proposed for any of these quantities; every entry is a measurement to be performed. 

|#|Parameter|Effort|Instrument|Why it is at this rank|
|---|---|---|---|---|
|1|Individual MQ/TGS heater<br>power_Pi_ and total_Q_gen|0.5 day|shunt + DMM, or in-<br>line power meter|Sets the entire thermal load; rank<br>4 in steady-state sensitivity; every<br>other parameter is identified rela-<br>tive to it|
|2|Peltier_α_,_Re_,_K_|1 hour<br>(datasheet) to<br>1 day (in situ)|datasheet<br>maxima;<br>pulsed<br>resistance;<br>open-circuit Seebeck|Rank 1 and 3 in steady-state sensi-<br>tivity;±20 %here is3 K to 6 Kof<br>model error [6, 14]|
|3|Wall material,<br>thickness,<br>mass_⇒Cw_, and_Gw∞_|0.5 day|balance; passive cool-<br>down curve|Rank 1 and 2 in_dynamic_sensitiv-<br>ity; together they set_τ_dom|
|4|Heater duty schedule|1 hour|firmware inspection +<br>oscilloscope|Decides whether_Q_genis a constant,<br>a filtered ripple or a disturbance<br>[7, 8]|
|5|Ambient temperature range<br>at the deployment site|continuous<br>logging|external logger|Passes to the chamber almost 1:1<br>open loop; sets both ends of the<br>reachable setpoint range|
|6|Heatsink thermal resistance<br>_R_th,hsat the actual airflow<br><sup>˙</sup>|0.5 day|datasheet + verification<br>by Eq. (26)|Protects the hot-side stability con-<br>dition; degrades in service [12]|
|7|Fan airflow _Vv_ and the air-<br>flow path|0.5 day|hot-wire anemometer;<br>smoke or tracer|Fixes<br>˙_m_ and the upper bound<br>_Gac ≤_˙_mcp_; resolves the G1/G2<br>ambiguity in practice|
|8|Chamber wall, cold-plate<br>and hot-side temperatures|1 day to fit|thermocouples or ther-<br>mistors|Without them the five-node model<br>can only be validated at its output,<br>which cannot distinguish a correct<br>model from compensating errors|
|9|Sensor-region temperature<br>and the gradient_Ts −Ta_|0.5 day|board-mounted<br>ther-<br>mistor|Decides whether node Option A of<br>Section 7 survives|
|10|Effective free-air volume<br>_V_air and internal wetted area|1 hour (CAD)|CAD subtraction|_V_airis thermally almost irrelevant,<br>but_A_intis not, and both come from<br>the same model|
|11|Internal surface emissivity_ε_|1 hour|finish specification|Decides whether radiation stays in<br>the reduced model (Section 11)|
|12|SHT20 response time_τm_|2 hours|two stirred baths + fine<br>thermocouple|The only phase lag in the loop; lim-<br>its achievable bandwidth|
|13|Dew point of the sample gas|2 hours|hygrometer|Sets the lower setpoint bound; a<br>violated bound destroys a measure-<br>ment|



Items 1–3 alone would convert the model from structurally correct to numerically usable. Items 8–9 are what make validation, as opposed to curve-fitting. Possible. 

## **36 Conclusion** 

A five-node lumped thermal model of an E-Nose chamber containing 14 MQ/TGS gas sensors and a Peltier forced-air cooling system has been derived from energy balances, extended with a measurement node, cast in nonlinear state-space form, discretised, implemented in Python, and used as the plant for a PID baseline and a Fuzzy-PID gain-adaptation layer. The derivation order was fixed in advance and never reversed: no reduced transfer function was assumed before the physics produced one. 

Four results of the derivation are worth carrying forward independently of any parameter value. The enclosed air stores about 0.47 J/K and is a coupling medium rather than a thermal store, so the effective freeair volume —the quantity the original specification emphasised most—changes the dominant time constant by 0.02 %; the measurement effort belongs on the wall and board masses instead. Linearised radiation at 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

47 

a high-emissivity internal surface is of the same order as both natural and fan-driven convection in this small, low-velocity chamber, so it may be folded into a combined surface coefficient but not discarded. The static characteristic from Peltier current to chamber temperature is strongly nonlinear, with the local gain varying by a factor of about three across the usable current range, which is the physical argument for gain adaptation. And with a unipolar drive the reachable setpoint set is bounded above by the passive equilibrium and below by the dew point, so the admissible setpoints are a hardware question before they are a control question. 

On the dry-run parameter set the Fuzzy-PID layer improved IAE by 19 % to 22 % and ITAE by 32 % to 36 % on cooling transitions at equal energy and effort, and reduced steady-state ripple by about a third; it was neutral for small disturbances and worse than the fixed-gain PID on the passive heating transition, where the actuator has no authority. That mixed outcome is reported as it stands. The hypothesis that Fuzzy-PID improves regulation for this plant is supported only in the direction where the actuator can act, and it remains to be tested on the calibrated model and on hardware. 

Finally, the limitation of the present document should be stated plainly: it establishes a model structure, not a property of the physical instrument. No sensor power, Peltier coefficient, airflow rate or material property has been measured, and every number reported from simulation carries a placeholder parameter set. Section 33 lists, in priority order, exactly what must be measured to close that gap. 

## **A Answers to the Critical Engineering Questions** 

Table 28: Status of each open question raised in the specification. 

|#|Question|Status and answer|
|---|---|---|
|1|Physical meaning of the34 mminner di-<br>ameter?|**Unresolved.** Four candidates in Sec. 2.2. Volume impact<br>26.3 mL(minor); airflow impact potentially large. Resolve<br>from CAD.|
|2|True free-air volume?|Bounded: 397 mL to 446 mL. The397 mLestimate implies<br>an implausible 5.4 % fill fraction under G1. Low thermal<br>priority (Sec. 27.2).|
|3|Heat generated by each MQ/TGS sen-<br>sor?|_Not available; determine experimentally or from the manu-_<br>_facturer._ Measure the array supply rail directly rather than<br>itemising.|
|4|Are all 14 MQ/TGS heaters continuously<br>active?|**Unknown, and it matters.** If pulsed,_Q_gen is periodic; mod-<br>ulation faster than10 sis filtered by the thermal mass, slower<br>modulation becomes a disturbance the controller must reject.<br>Read the firmware schedule.|
|5|Is the NDIR sensor inside the controlled<br>volume?|Assumed yes in Eq.(9). If it sits outside, remove_Q_NDIR and<br>add a conduction path through its mounting.|
|6|Is the fan inside the chamber or only in<br>the cooling section?|**Unknown.** Determines whether _Q_fan appears in _Q_gen and<br>whether the cylinder sees forced or natural convection — a<br>factor of about 1.3 in_h_either way.|
|7|Actual airflow rate?|_Not available; measure with a hot-wire anemometer._ The<br>1 m/sused in Sec. 10 is an assumption for order-of-magnitude<br>purposes only.|
|8|Airflow path?|**Unknown**, and coupled to question 1. Determines whether<br>the box and the cylinder are in series or in parallel thermally.|
|9–<br>10|Location of the Peltier cold and hot<br>sides?|Assumed: cold side in the upper box, hot side outside the<br>enclosure with the heatsink and fan. If the hot side is inside<br>any part of the controlled envelope, the model is wrong at<br>the topology level, not merely in its parameters.|
|11|Heatsink thermal resistance?|_Not available._ Obtain _R_th(_v_) from the heatsink datasheet<br>at the measured airflow, then verify with Eq. (26). This<br>parameter protects the stability condition of Sec. 15.|



E-Nose Chamber Thermal Model and Fuzzy-PID Control 

48 

|#|Question|Status and answer|
|---|---|---|
|12|Ambient operating range?|**Must be specified.**Ambient passes to the chamber almost 1:1<br>open-loop (Experiment E) and sets both ends of the reachable<br>range. A substation environment is not a laboratory.|
|13|Control sensor temperature or chamber-<br>air temperature?|**Answered: control**_Ta_ **(measured), monitor**_Ts_**.** Reasoning<br>in Sec. 14.2. Cascade control is the natural upgrade once_Ts_<br>is instrumented.|
|14|How large is the internal thermal gradi-<br>ent?|Dry-run predicts_Ts−Ta ≈_15 Kto20 K, strongly dependent<br>on_Q_gen. Must be measured; it is the quantity that shifts the<br>sensor baseline.|
|15|Is a single SHT20 reading sufficient?|**Probably not.** The lumped-air assumption (A2) is the weak-<br>est in the model. Use both SHT20 units at different locations<br>and report their difference as a validity check on A2.|
|16|Are there hotspots near individual<br>MQ/TGS sensors?|Certain to exist; not represented by any lumped model. De-<br>tect with a thermal camera on an open build, or accept and<br>characterise the resulting sensor-to-sensor baseline spread.|
|17|How does chamber temperature affect<br>gas-sensor response?|Outside the thermal model’s scope, but it is the reason the<br>model exists. Quantify by holding a fixed gas concentration<br>and stepping the setpoint across the admissible range, record-<br>ing the response of all 14 channels. This experiment also sets<br>the required control precision.|
|18|Is temperature stabilisation required be-<br>fore gas measurement?|Almost certainly yes. With _τ_dom of order 250 s, allow a<br>settling window of3 to 5time constants (12 min to 20 min)<br>after any setpoint change, or gate the measurement on_|e| <_<br>a stated threshold.|
|19|What setpoint range is compatible with<br>both thermal control and sensing?|The intersection of[_T_dew+ ∆_, Ta_(_I_ = 0)](Sec. 21.4) with<br>the range over which the sensor response is well behaved<br>(question 17). Neither interval is known yet; both are cheap<br>to measure.|



## **B Reference Implementation** 

The complete implementation used to produce every figure and table in Sections 21–28 is reproduced below in condensed form. It is deliberately written so that replacing the single dictionary P with measured values regenerates the entire analysis. 

"""E-Nose chamber thermal model: 5 thermal nodes + measurement node, Peltier actuator, PID and Fuzzy-PID. All values in dict P are PLACEHOLDERS. """ import numpy as np from scipy.optimize import fsolve, curve_fit P = dict( C_s=40.0, # sensor array + PCB [J/K] PLACEHOLDER C_a=0.4724, # chamber air = rho*V*cp [J/K] derived C_w=150.0, # chamber wall [J/K] PLACEHOLDER C_c=25.0, # cold-side assembly [J/K] PLACEHOLDER C_h=90.0, # hot-side assembly [J/K] PLACEHOLDER G_sa=0.30, # sensor -> air [W/K] PLACEHOLDER G_sw=0.15, # sensor/PCB -> wall (cond.) [W/K] PLACEHOLDER G_aw=0.40, # air -> wall [W/K] PLACEHOLDER G_ac=2.50, # air -> cold heatsink [W/K] PLACEHOLDER G_wo=0.50, # wall -> ambient [W/K] PLACEHOLDER G_ho=2.86, # hot heatsink -> ambient [W/K] PLACEHOLDER (Rth=0.35 K/W) alpha=0.053, # Seebeck, module [V/K] PLACEHOLDER Re_p=1.90, # electrical resistance [ohm] PLACEHOLDER K_p=0.62, # thermal conductance [W/K] PLACEHOLDER Imax=4.0, # actuator saturation [A] tau_m=15.0, # SHT20 measurement lag [s] PLACEHOLDER ) 

Q_GEN_NOM = 9.0 # W PLACEHOLDER total internal dissipation 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

49 

T_AMB_NOM = 25.0 # degC def peltier(I, Tc_C, Th_C, p): """Cold- and hot-side heat flows [W]. Absolute temperatures required.""" Tc, Th = Tc_C + 273.15, Th_C + 273.15 Qc = p["alpha"] * I * Tc - 0.5 * I**2 * p["Re_p"] - p["K_p"] * (Th - Tc) Qh = p["alpha"] * I * Th + 0.5 * I**2 * p["Re_p"] - p["K_p"] * (Th - Tc) return Qc, Qh def f(x, I, Qgen, Tamb, p): Ts, Ta, Tw, Tc, Th, Tm = x Qc, Qh = peltier(I, Tc, Th, p) dTs = (Qgen - p["G_sa"]*(Ts-Ta) - p["G_sw"]*(Ts-Tw)) / p["C_s"] dTa = (p["G_sa"]*(Ts-Ta) - p["G_aw"]*(Ta-Tw) - p["G_ac"]*(Ta-Tc)) / p["C_a"] dTw = (p["G_aw"]*(Ta-Tw) + p["G_sw"]*(Ts-Tw) - p["G_wo"]*(Tw-Tamb)) / p["C_w"] dTc = (p["G_ac"]*(Ta-Tc) - Qc) / p["C_c"] dTh = (Qh - p["G_ho"]*(Th-Tamb)) / p["C_h"] dTm = (Ta - Tm) / p["tau_m"] # SHT20 sensor dynamics return np.array([dTs, dTa, dTw, dTc, dTh, dTm]) def rk4(x, I, Qgen, Tamb, p, dt): k1 = f(x, I, Qgen, Tamb, p) k2 = f(x + 0.5*dt*k1, I, Qgen, Tamb, p) k3 = f(x + 0.5*dt*k2, I, Qgen, Tamb, p) k4 = f(x + dt*k3, I, Qgen, Tamb, p) return x + dt/6.0*(k1 + 2*k2 + 2*k3 + k4) def steady_state(I, Qgen, Tamb, p): """Algebraic equilibrium f(x)=0 (Newton). Time-marching to steady state is avoided because the air node is ~3 orders of magnitude faster than the wall node (stiff system).""" x0 = np.array([Tamb+20, Tamb+10, Tamb+5, Tamb, Tamb+15, Tamb+10], float) sol = fsolve(lambda x: f(x, I, Qgen, Tamb, p), x0, full_output=True) return sol[0] # ---------------------------------------------------------------- controllers class PID: """u = -(Kp e + Ki int e + Kd de/dt); the plant gain dT/dI is negative.""" def __init__(self, Kp, Ki, Kd, Ts, umin=0.0, umax=4.0, N=8.0, b=1.0): self.Kp0, self.Ki0, self.Kd0 = Kp, Ki, Kd self.Kp, self.Ki, self.Kd = Kp, Ki, Kd self.Ts, self.umin, self.umax, self.N = Ts, umin, umax, N self.b = b self.I = 0.0 self.dfil = 0.0 self.yprev = None self.sp = 0.0 def __call__(self, t, x): y = x[5] # measured (SHT20) temperature e = self.sp - y dy = 0.0 if self.yprev is None else (y - self.yprev)/self.Ts Tf = max(self.Kd/max(self.Kp, 1e-9)/self.N, self.Ts) self.dfil += (self.Ts/Tf)*(dy - self.dfil) self.yprev = y self.adapt(e, -self.dfil) # derivative acting on the measurement (no derivative kick), # set-point weighting b on the proportional term u_raw = -(self.Kp*(self.b*self.sp - y) + self.I - self.Kd*self.dfil) u = float(np.clip(u_raw, self.umin, self.umax)) if (u == u_raw) or ((u_raw - u) * (-e) < 0): # conditional integration self.I += self.Ki*e*self.Ts return u def adapt(self, e, de): pass def trimf(v, a, b, c): if v <= a or v >= c: return 0.0 if v == b: 

50 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

return 1.0 return (v-a)/(b-a) if v < b else (c-v)/(c-b) CENT = np.array([-1.0, -2/3, -1/3, 0.0, 1/3, 2/3, 1.0]) LBL = ["NB", "NM", "NS", "ZO", "PS", "PM", "PB"] def mfs(v): v = float(np.clip(v, -1, 1)) mu = np.zeros(7) for i, c in enumerate(CENT): a = CENT[i-1] if i > 0 else -2.0 b = CENT[i+1] if i < 6 else 2.0 mu[i] = trimf(v, a, c, b) return mu S = {"NB": -1.0, "NM": -2/3, "NS": -1/3, "ZO": 0.0, "PS": 1/3, "PM": 2/3, "PB": 1.0} RKP = [["PB", "PB", "PM", "PM", "PS", "ZO", "ZO"], ["PB", "PB", "PM", "PS", "PS", "ZO", "NS"], ["PM", "PM", "PM", "PS", "ZO", "NS", "NS"], ["PM", "PM", "PS", "ZO", "NS", "NM", "NM"], ["PS", "PS", "ZO", "NS", "NS", "NM", "NM"], ["PS", "ZO", "NS", "NM", "NM", "NM", "NB"], ["ZO", "ZO", "NM", "NM", "NM", "NB", "NB"]] RKI = [["NB", "NB", "NM", "NM", "NS", "ZO", "ZO"], ["NB", "NB", "NM", "NS", "NS", "ZO", "ZO"], ["NB", "NM", "NS", "NS", "ZO", "PS", "PS"], ["NM", "NM", "NS", "ZO", "PS", "PM", "PM"], ["NM", "NS", "ZO", "PS", "PS", "PM", "PB"], ["ZO", "ZO", "PS", "PS", "PM", "PB", "PB"], ["ZO", "ZO", "PS", "PM", "PM", "PB", "PB"]] RKD = [["PS", "NS", "NB", "NB", "NB", "NM", "PS"], ["PS", "NS", "NB", "NM", "NM", "NS", "ZO"], ["ZO", "NS", "NM", "NM", "NS", "NS", "ZO"], ["ZO", "NS", "NS", "NS", "NS", "NS", "ZO"], ["ZO", "ZO", "ZO", "ZO", "ZO", "ZO", "ZO"], ["PB", "PS", "PS", "PS", "PS", "PS", "PB"], ["PB", "PM", "PM", "PM", "PS", "PS", "PB"]] CKP = np.array([[S[RKP[i][j]] for j in range(7)] for i in range(7)]) CKI = np.array([[S[RKI[i][j]] for j in range(7)] for i in range(7)]) CKD = np.array([[S[RKD[i][j]] for j in range(7)] for i in range(7)]) def fuzzy_infer(en, den): W = np.outer(mfs(en), mfs(den)) s = W.sum() if s < 1e-9: return 0.0, 0.0, 0.0 return (float((W*CKP).sum()/s), float((W*CKI).sum()/s), float((W*CKD).sum()/s)) class FuzzyPID(PID): def __init__(self, *a, Ke=2.5, Kde=0.08, aKp=0.7, aKi=0.7, aKd=0.7, **kw): super().__init__(*a, **kw) self.Ke, self.Kde = Ke, Kde self.aKp, self.aKi, self.aKd = aKp, aKi, aKd self.trace = [] def adapt(self, e, de): en = float(np.clip(e/self.Ke, -1, 1)) den = float(np.clip(de/self.Kde, -1, 1)) dp, di, dd = fuzzy_infer(en, den) self.Kp = max(0.0, self.Kp0*(1 + self.aKp*dp)) self.Ki = max(0.0, self.Ki0*(1 + self.aKi*di)) self.Kd = max(0.0, self.Kd0*(1 + self.aKd*dd)) self.trace.append((self.Kp, self.Ki, self.Kd)) 

## **C Note on Reference Selection and Verification** 

The 21 references were selected to support specific mathematical or engineering arguments rather than to establish topical coverage, and each is cited at the point where it is used. The set spans the required areas: 

51 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

thermoelectric modelling and parameter extraction [6, 14, 15]; lumped-parameter thermal networks and their calibration [9–11]; forced-convection and heatsink thermal resistance [12, 13]; metal-oxide sensor heater behaviour, drift and pulsed operation [1, 2, 7, 8]; E-Nose systems for transformer DGA [3–5, 20, 21]; and PID tuning and fuzzy or adaptive temperature control [16–19]. 

Author lists, titles, journals, publishers, volumes, pages, years and DOIs were retrieved from the Crossref registry and are reproduced as returned; none was written from memory. Eighteen of the twentyone were published in the last five years. The three exceptions are deliberate: [16] is the primary source for the SIMC tuning rules actually used in Section 21, and citing a derivative work in its place would be inaccurate attribution. 

Publishers represented are Elsevier (5), IEEE (5), MDPI (5), Springer Nature (1), Taylor & Francis / Informa (1) and IOP (2). **Journal quartile rankings are not asserted anywhere in this document. Quartile status should be verified through the current Scopus/SCImago database at the time of submission** , since quartiles change annually and any figure quoted here would soon be out of date. 

## **References** 

- [1] Tarik Saidi, Abderrazak Manser, and Tesfalem Welearegay. Heater power supply fluctuations in metal oxide gas sensors: impact on gas sensing performance. _Engineering Research Express_ , 6(3):035230, 2024. doi: 10.1088/2631-8695/ad734f. 

- [2] Abdulnasser Nabil Abdullah, Kamarulzaman Kamarudin, Latifah Munirah Kamarudin, Abdul Hamid Adom, Syed Muhammad Mamduh, Zaffry Hadi Mohd Juffry, and Victor Hernandez Bennetts. Correction model for metal oxide sensor drift caused by ambient temperature and humidity. _Sensors_ , 22 (9):3301, 2022. doi: 10.3390/s22093301. 

- [3] Suganya Govindarajan, Harimurugan Devarajan, Jorge Alfredo Ardila-Rey, Matías Patricio CerdaLuna, Sergi Leandro Torres Araya, and Cristhian Camilo Delgado Diaz. Intelligent interpretation of dissolved gases in transformer oil with electronic nose and machine learning. _IEEE Transactions on Industrial Informatics_ , 21(4):2839–2848, 2025. doi: 10.1109/tii.2024.3507943. 

- [4] Sergi Torres Araya, Jorge Ardila-Rey, Matías Cerda Luna, Jorge Portilla, Suganya Govindarajan, Camilo Alvear Jorquera, and Roger Schurch. Performance assessment of machine learning techniques in electronic nose systems for power transformer fault detection. _Energy and AI_ , 20:100497, 2025. doi: 10.1016/j.egyai.2025.100497. 

- [5] Jorge Alfredo Ardila-Rey, Matías Patricio Cerda-Luna, Carlos Beltran Muñoz, Bruno Albuquerque de Castro, and Suganya Govindarajan. A novel e-nose system for the characterization of dissolved gases in dielectric oils. _IEEE Transactions on Instrumentation and Measurement_ , 72:1–16, 2023. doi: 10.1109/tim.2023.3307177. 

- [6] Enzo Evers, Rens Slenders, Rob van Gils, Bram de Jager, and Tom Oomen. Thermoelectric modules in mechatronic systems: Temperature-dependent modeling and control. _Mechatronics_ , 79:102647, 2021. doi: 10.1016/j.mechatronics.2021.102647. 

- [7] Francisco Palacio, Jordi Fonollosa, Javier Burgues, Jose M. Gomez, and Santiago Marco. Pulsedtemperature metal oxide gas sensors for microwatt power consumption. _IEEE Access_ , 8:70938–70946, 2020. doi: 10.1109/access.2020.2987066. 

- [8] Yu Bing, Fuyun Zhang, Jiatong Han, Tingting Zhou, Haixia Mei, and Tong Zhang. A method of ultra-low power consumption implementation for mems gas sensors. _Chemosensors_ , 11(4):236, 2023. doi: 10.3390/chemosensors11040236. 

- [9] Jon García Urbieta, Borja Rodríguez, Antonio J. Rodríguez, Pablo Díaz, Sergio Armentia, and Francisco González. Sensitivity analysis of lumped-parameter thermal networks for the experimental 

52 

E-Nose Chamber Thermal Model and Fuzzy-PID Control 

calibration of emotor models. _IEEE Transactions on Transportation Electrification_ , 10(3):6210–6220, 2024. doi: 10.1109/tte.2023.3331097. 

- [10] Yue Fan, Wei Feng, Zhenxing Ren, Bingqi Liu, and Dazhi Wang. Lumped parameter thermal network modeling and thermal optimization design of an aerial camera. _Sensors_ , 24(12):3982, 2024. doi: 10.3390/s24123982. 

- [11] Anshuman Dey, Navid Shafiei, Ri Li, Wilson Eberle, and Rahul Khandekar. Boundary condition independent thermal network modeling of high-frequency power transformers. _Heat Transfer Engineering_ , 44(3):259–276, 2022. doi: 10.1080/01457632.2022.2049548. 

- [12] Jianxiong Yu, Kaiyang Bu, Ye Tian, Bowen Liu, Yongjun Zheng, Bin Guo, and Chushan Li. Analytical modeling of forced-air cooling heatsink by considering air temperature rise in channel and coupling of multiple heat sources. _IEEE Access_ , 13:128685–128699, 2025. doi: 10.1109/access.2025.3589206. 

- [13] Pengyang Qu, Jianjie Cheng, Yurong Chen, Yannan Li, Wei Li, and Hanzhong Tao. Numerical and experimental investigation on heat transfer of multi-heat sources mounted on a fined radiator within embedded heat pipes in an electronic cabinet. _International Journal of Thermal Sciences_ , 183:107833, 2023. doi: 10.1016/j.ijthermalsci.2022.107833. 

- [14] Hanlong Wan, Bo Shen, and Zhenning Li. Parameter extraction approaches for compact modeling of thermoelectric modules. _International Journal of Heat and Mass Transfer_ , 225:125366, 2024. doi: 10.1016/j.ijheatmasstransfer.2024.125366. 

- [15] Atmanandmaya, Umanand Loganathan, and Subba Reddy B. Dynamic modeling and physical insights into peltier module behavior enabling a foundation for model-based control in power electronics. _IEEE Transactions on Industry Applications_ , 62(4):6680–6693, 2026. doi: 10.1109/tia.2025.3644991. 

- [16] Sigurd Skogestad. Simple analytic rules for model reduction and pid controller tuning. _Journal of Process Control_ , 13(4):291–309, 2003. doi: 10.1016/s0959-1524(02)00062-8. 

- [17] Lu Liu, Dingyu Xue, and Shuo Zhang. General type industrial temperature system control based on fuzzy fractional-order pid controller. _Complex & Intelligent Systems_ , 9(3):2585–2597, 2021. doi: 10.1007/s40747-021-00431-9. 

- [18] Miguel F. Ferrer Pareja, Carlos Sánchez Morales, Federico León Zerpa, and Alejandro Ramos Martín. Hybrid pso-tuned fractional-order control with rule-based adaptive supervision for embedded thermoelectric temperature regulation. _Fractal and Fractional_ , 10(4):238, 2026. doi: 10.3390/ fractalfract10040238. 

- [19] Juan Carlos Almachi, Ramiro Vicente, Edwin Bone, Jessica Montenegro, Edgar Cando, and Salvatore Reina. Implementation of a neural network for adaptive pid tuning in a high-temperature thermal system. _Energies_ , 18(12):3113, 2025. doi: 10.3390/en18123113. 

- [20] Mohd Syukri Ali, Ab Halim Abu Bakar, Azimah Omar, Amirul Syafiq Abdul Jaafar, and Siti Hajar Mohamed. Conventional methods of dissolved gas analysis using oil-immersed power transformer for fault diagnosis: A review. _Electric Power Systems Research_ , 216:109064, 2023. doi: 10.1016/j. epsr.2022.109064. 

- [21] Haixia Mei, Jingyi Peng, Dongdong Xu, and Tao Wang. Low-power chemiresistive gas sensors for transformer fault diagnosis. _Molecules_ , 29(19):4625, 2024. doi: 10.3390/molecules29194625. 

