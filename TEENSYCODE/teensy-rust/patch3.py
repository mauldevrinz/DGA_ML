import re

with open("src/main.rs", "r") as f:
    code = f.read()

loop_start_regex = re.compile(r'        for gpio_idx in 0\.\.6 \{\n            // Cek perintah setpoint suhu dari host.*?usb_println!\("ACK SETPOINT = \{\:\.2\} C", setpoint_c\);\n            \}', re.DOTALL)

loop_start_new = """        for gpio_idx in 0..6 {
            if let Some(cmd) =
                with_usb_comm(|comm| comm.poll_cmd(&mut setpoint_rx_buf, &mut setpoint_rx_len))
                    .flatten()
            {
                match cmd {
                    HostCmd::Setpoint(val) => {
                        setpoint_c = val;
                        usb_println!("ACK SETPOINT = {:.2} C", setpoint_c);
                    },
                    HostCmd::Phase(p) => {
                        phase_mode = p;
                        usb_println!("ACK PHASE = {}", p);
                    },
                    HostCmd::Pump1Pwm(val) => {
                        let val_permille = (val.min(100) as u16) * 10;
                        pump1_pwm_val = val_permille;
                        usb_println!("ACK PWM = {}%", val.min(100));
                        if phase_mode == "INJECT" {
                            set_pump1_pwm(pump1_pwm_val);
                        }
                    }
                }
            }
            
            // Handle Phases
            match phase_mode {
                "IDLE" => {
                    let _ = p14.set_high(); // Pump 2
                    let _ = p38.set_high(); // SV1
                    let _ = p39.set_high(); // SV2
                    
                    let _ = p40.set_low(); // SV3
                    let _ = p41.set_low(); // Pump 3
                    set_pump1_pwm(0);
                },
                "INJECT" => {
                    set_pump1_pwm(pump1_pwm_val); // Pump 1
                    let _ = p40.set_high(); // SV3
                    
                    let _ = p14.set_low(); // Pump 2
                    let _ = p41.set_low(); // Pump 3
                    let _ = p38.set_low(); // SV1
                    let _ = p39.set_low(); // SV2
                },
                "PURGE" => {
                    let _ = p41.set_high(); // Pump 3
                    let _ = p39.set_high(); // SV2
                    let _ = p40.set_high(); // SV3
                    
                    let _ = p38.set_low(); // SV1
                    let _ = p14.set_low(); // Pump 2
                    set_pump1_pwm(0);
                },
                _ => { // OFF
                    let _ = p14.set_low();
                    let _ = p38.set_low();
                    let _ = p39.set_low();
                    let _ = p40.set_low();
                    let _ = p41.set_low();
                    set_pump1_pwm(0);
                }
            }"""

code = loop_start_regex.sub(loop_start_new, code)

# Remove the old MATIKAN SEMUA GPIO and NYALAKAN GPIO AKTIF
code = re.sub(r'\s*// MATIKAN SEMUA GPIO.*?match gpio_idx \{.*?\n            \}', '', code, flags=re.DOTALL)

with open("src/main.rs", "w") as f:
    f.write(code)

