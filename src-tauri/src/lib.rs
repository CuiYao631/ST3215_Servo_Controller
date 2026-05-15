mod servo_protocol;

use serde::{Deserialize, Serialize};
use serialport::{SerialPort, SerialPortType};
use servo_protocol::*;
use std::sync::Mutex;
use std::time::Duration;
use tauri::State;

struct AppState {
    port: Mutex<Option<Box<dyn SerialPort>>>,
}

#[derive(Serialize, Deserialize)]
struct PortInfo {
    name: String,
    port_type: String,
}

#[derive(Serialize, Deserialize)]
struct ServoStatus {
    position: i16,
    speed: i16,
    load: i16,
    voltage: f32,
    temperature: u8,
    current: f32,
    moving: bool,
    error: u8,
}

/// 列出所有可用串口
#[tauri::command]
fn list_ports() -> Result<Vec<PortInfo>, String> {
    let ports = serialport::available_ports().map_err(|e| e.to_string())?;
    Ok(ports
        .into_iter()
        .map(|p| PortInfo {
            name: p.port_name.clone(),
            port_type: match p.port_type {
                SerialPortType::UsbPort(info) => {
                    format!("USB - {}", info.product.unwrap_or_default())
                }
                SerialPortType::BluetoothPort => "Bluetooth".into(),
                SerialPortType::PciPort => "PCI".into(),
                _ => "Unknown".into(),
            },
        })
        .collect())
}

/// 连接串口 (波特率1M)
#[tauri::command]
fn connect_port(port_name: String, baud_rate: u32, state: State<AppState>) -> Result<(), String> {
    // 先断开已有连接
    {
        let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
        *port_lock = None;
    }

    let port = serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| format!("打开串口失败: {}", e))?;

    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    *port_lock = Some(port);
    Ok(())
}

/// 断开串口
#[tauri::command]
fn disconnect_port(state: State<AppState>) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    *port_lock = None;
    Ok(())
}

/// 检查串口连接状态
#[tauri::command]
fn is_connected(state: State<AppState>) -> bool {
    state.port.lock().map(|p| p.is_some()).unwrap_or(false)
}

/// 发送数据并接收应答
fn send_and_receive(
    port: &mut Box<dyn SerialPort>,
    packet: &[u8],
    expect_response: bool,
) -> Result<Vec<u8>, String> {
    // 清空接收缓冲区（非阻塞方式）
    port.set_timeout(Duration::from_millis(1))
        .map_err(|e| format!("设置超时失败: {}", e))?;
    let mut discard = [0u8; 512];
    loop {
        match port.read(&mut discard) {
            Ok(0) => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    }
    // 恢复正常超时
    port.set_timeout(Duration::from_millis(50))
        .map_err(|e| format!("设置超时失败: {}", e))?;

    port.write_all(packet)
        .map_err(|e| format!("发送失败: {}", e))?;
    port.flush().map_err(|e| format!("刷新失败: {}", e))?;

    if !expect_response {
        return Ok(vec![]);
    }

    std::thread::sleep(Duration::from_millis(5));

    let mut response = Vec::new();
    let mut buf = [0u8; 256];
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(100);

    while start.elapsed() < timeout {
        match port.read(&mut buf) {
            Ok(n) if n > 0 => {
                response.extend_from_slice(&buf[..n]);
                // 检查是否收到完整的包
                if response.len() >= 6 {
                    if response[0] == 0xFF && response[1] == 0xFF {
                        let length = response[3] as usize;
                        if response.len() >= 4 + length {
                            break;
                        }
                    }
                }
            }
            Ok(_) => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                if !response.is_empty() {
                    break;
                }
                continue;
            }
            Err(e) => return Err(format!("接收失败: {}", e)),
        }
    }

    if response.is_empty() {
        return Err("未收到应答".to_string());
    }

    Ok(response)
}

/// PING舵机
#[tauri::command]
fn ping_servo(id: u8, state: State<AppState>) -> Result<u8, String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_ping(id);
    let response = send_and_receive(port, &packet, true)?;
    let (_, error, _) = parse_response(&response)?;
    Ok(error)
}

/// 读取寄存器值
#[tauri::command]
fn read_register(id: u8, addr: u8, len: u8, state: State<AppState>) -> Result<Vec<u8>, String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_read(id, addr, len);
    let response = send_and_receive(port, &packet, true)?;
    let (_, error, data) = parse_response(&response)?;
    if error != 0 {
        return Err(format!("舵机错误: 0x{:02X}", error));
    }
    Ok(data)
}

/// 写入寄存器值
#[tauri::command]
fn write_register(id: u8, addr: u8, data: Vec<u8>, state: State<AppState>) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_write(id, addr, &data);
    let _response = send_and_receive(port, &packet, true)?;
    Ok(())
}

/// 控制舵机转动 - 设置目标位置和速度
#[tauri::command]
fn move_servo(
    id: u8,
    position: i16,
    speed: u16,
    acceleration: u8,
    state: State<AppState>,
) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    // 先打开扭矩
    let torque_packet = build_write(id, ADDR_TORQUE_SWITCH, &[1]);
    send_and_receive(port, &torque_packet, true)?;

    // 设置加速度
    if acceleration > 0 {
        let acc_packet = build_write(id, ADDR_ACCELERATION, &[acceleration]);
        send_and_receive(port, &acc_packet, true)?;
    }

    // 写入目标位置(2字节) + 运行时间(2字节) + 运行速度(2字节)
    let pos_low = (position & 0xFF) as u8;
    let pos_high = ((position >> 8) & 0xFF) as u8;
    let speed_low = (speed & 0xFF) as u8;
    let speed_high = ((speed >> 8) & 0xFF) as u8;

    let data = vec![pos_low, pos_high, 0, 0, speed_low, speed_high];
    let packet = build_write(id, ADDR_TARGET_POSITION, &data);
    let _response = send_and_receive(port, &packet, true)?;

    Ok(())
}

/// 读取舵机实时状态
#[tauri::command]
fn read_servo_status(id: u8, state: State<AppState>) -> Result<ServoStatus, String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    // 从地址56开始读取15字节 (位置2+速度2+负载2+电压1+温度1+异步写1+状态1+移动1+空2+电流2)
    let packet = build_read(id, ADDR_CURRENT_POSITION, 15);
    let response = send_and_receive(port, &packet, true)?;
    let (_, error, data) = parse_response(&response)?;

    if data.len() < 15 {
        return Err(format!("数据不完整: {} 字节", data.len()));
    }

    let position = (data[0] as i16) | ((data[1] as i16) << 8);
    let speed = (data[2] as i16) | ((data[3] as i16) << 8);
    let load = (data[4] as i16) | ((data[5] as i16) << 8);
    let voltage = data[6] as f32 * 0.1;
    let temperature = data[7];
    let moving = data[10] != 0;
    let current = if data.len() >= 15 {
        let raw = (data[13] as u16) | ((data[14] as u16) << 8);
        raw as f32 * 6.5
    } else {
        0.0
    };

    Ok(ServoStatus {
        position,
        speed,
        load,
        voltage,
        temperature,
        current,
        moving,
        error,
    })
}

/// 设置扭矩开关
#[tauri::command]
fn set_torque(id: u8, enable: u8, state: State<AppState>) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_write(id, ADDR_TORQUE_SWITCH, &[enable]);
    send_and_receive(port, &packet, true)?;
    Ok(())
}

/// 解锁EPROM写入
#[tauri::command]
fn unlock_eprom(id: u8, state: State<AppState>) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_write(id, ADDR_LOCK_FLAG, &[0]);
    send_and_receive(port, &packet, true)?;
    Ok(())
}

/// 锁定EPROM
#[tauri::command]
fn lock_eprom(id: u8, state: State<AppState>) -> Result<(), String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;

    let packet = build_write(id, ADDR_LOCK_FLAG, &[1]);
    send_and_receive(port, &packet, true)?;
    Ok(())
}

/// 获取内存表定义
#[tauri::command]
fn get_register_info() -> Vec<RegisterInfo> {
    get_register_table()
}

/// 读取所有寄存器
#[tauri::command]
fn read_all_registers(id: u8, state: State<AppState>) -> Result<Vec<(u8, Vec<u8>)>, String> {
    let mut port_lock = state.port.lock().map_err(|e| e.to_string())?;
    let port = port_lock.as_mut().ok_or("串口未连接")?;
    let regs = get_register_table();
    let mut results = Vec::new();

    for reg in &regs {
        let packet = build_read(id, reg.address, reg.bytes);
        match send_and_receive(port, &packet, true) {
            Ok(response) => match parse_response(&response) {
                Ok((_, _, data)) => results.push((reg.address, data)),
                Err(_) => results.push((reg.address, vec![])),
            },
            Err(_) => results.push((reg.address, vec![])),
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    Ok(results)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            port: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            list_ports,
            connect_port,
            disconnect_port,
            is_connected,
            ping_servo,
            read_register,
            write_register,
            move_servo,
            read_servo_status,
            set_torque,
            unlock_eprom,
            lock_eprom,
            get_register_info,
            read_all_registers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
