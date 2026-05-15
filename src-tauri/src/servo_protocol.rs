use serde::{Deserialize, Serialize};

/// ST3215 舵机协议实现
/// 通讯协议：0xFF 0xFF ID Length Instruction Parameters Checksum

// 指令类型
pub const INST_PING: u8 = 0x01;
pub const INST_READ: u8 = 0x02;
pub const INST_WRITE: u8 = 0x03;
pub const INST_REG_WRITE: u8 = 0x04;
pub const INST_ACTION: u8 = 0x05;
pub const INST_RESET: u8 = 0x06;
pub const INST_SYNC_READ: u8 = 0x82;
pub const INST_SYNC_WRITE: u8 = 0x83;

// 内存表地址定义 - EPROM区 (掉电保存)
pub const ADDR_FIRMWARE_MAJOR: u8 = 0;
pub const ADDR_FIRMWARE_MINOR: u8 = 1;
pub const ADDR_SERVO_MAJOR: u8 = 3;
pub const ADDR_SERVO_MINOR: u8 = 4;
pub const ADDR_ID: u8 = 5;
pub const ADDR_BAUD_RATE: u8 = 6;
pub const ADDR_RETURN_DELAY: u8 = 7;
pub const ADDR_RESPONSE_LEVEL: u8 = 8;
pub const ADDR_MIN_ANGLE: u8 = 9;
pub const ADDR_MAX_ANGLE: u8 = 11;
pub const ADDR_MAX_TEMP: u8 = 13;
pub const ADDR_MAX_VOLTAGE: u8 = 14;
pub const ADDR_MIN_VOLTAGE: u8 = 15;
pub const ADDR_MAX_TORQUE: u8 = 16;
pub const ADDR_PHASE: u8 = 18;
pub const ADDR_UNLOAD_COND: u8 = 19;
pub const ADDR_LED_ALARM: u8 = 20;
pub const ADDR_POS_P: u8 = 21;
pub const ADDR_POS_D: u8 = 22;
pub const ADDR_POS_I: u8 = 23;
pub const ADDR_MIN_STARTUP_FORCE: u8 = 24;
pub const ADDR_INTEGRAL_LIMIT: u8 = 25;
pub const ADDR_CW_DEAD_ZONE: u8 = 26;
pub const ADDR_CCW_DEAD_ZONE: u8 = 27;
pub const ADDR_PROTECT_CURRENT: u8 = 28;
pub const ADDR_ANGLE_RESOLUTION: u8 = 30;
pub const ADDR_POS_CORRECTION: u8 = 31;
pub const ADDR_OPERATION_MODE: u8 = 33;
pub const ADDR_PROTECT_TORQUE: u8 = 34;
pub const ADDR_PROTECT_TIME: u8 = 35;
pub const ADDR_OVERLOAD_TORQUE: u8 = 36;
pub const ADDR_SPEED_P: u8 = 37;
pub const ADDR_OVERCURRENT_TIME: u8 = 38;
pub const ADDR_SPEED_I: u8 = 39;

// 内存表地址定义 - SRAM区 (掉电不保存)
pub const ADDR_TORQUE_SWITCH: u8 = 40;
pub const ADDR_ACCELERATION: u8 = 41;
pub const ADDR_TARGET_POSITION: u8 = 42;
pub const ADDR_RUNNING_TIME: u8 = 44;
pub const ADDR_RUNNING_SPEED: u8 = 46;
pub const ADDR_TORQUE_LIMIT: u8 = 48;
pub const ADDR_LOCK_FLAG: u8 = 55;
pub const ADDR_CURRENT_POSITION: u8 = 56;
pub const ADDR_CURRENT_SPEED: u8 = 58;
pub const ADDR_CURRENT_LOAD: u8 = 60;
pub const ADDR_CURRENT_VOLTAGE: u8 = 62;
pub const ADDR_CURRENT_TEMP: u8 = 63;
pub const ADDR_ASYNC_WRITE_FLAG: u8 = 64;
pub const ADDR_SERVO_STATUS: u8 = 65;
pub const ADDR_MOVING_FLAG: u8 = 66;
pub const ADDR_CURRENT_CURRENT: u8 = 69;

pub const BROADCAST_ID: u8 = 0xFE;

/// 计算校验和: ~(ID + Length + Instruction + Params) & 0xFF
pub fn calc_checksum(id: u8, length: u8, instruction: u8, params: &[u8]) -> u8 {
    let mut sum: u16 = id as u16 + length as u16 + instruction as u16;
    for p in params {
        sum += *p as u16;
    }
    (!sum as u8) & 0xFF
}

/// 构建指令包
pub fn build_packet(id: u8, instruction: u8, params: &[u8]) -> Vec<u8> {
    let length = (params.len() + 2) as u8;
    let checksum = calc_checksum(id, length, instruction, params);
    let mut packet = vec![0xFF, 0xFF, id, length, instruction];
    packet.extend_from_slice(params);
    packet.push(checksum);
    packet
}

/// 构建PING指令包
pub fn build_ping(id: u8) -> Vec<u8> {
    build_packet(id, INST_PING, &[])
}

/// 构建读指令包
pub fn build_read(id: u8, start_addr: u8, read_len: u8) -> Vec<u8> {
    build_packet(id, INST_READ, &[start_addr, read_len])
}

/// 构建写指令包
pub fn build_write(id: u8, start_addr: u8, data: &[u8]) -> Vec<u8> {
    let mut params = vec![start_addr];
    params.extend_from_slice(data);
    build_packet(id, INST_WRITE, &params)
}

/// 构建异步写指令包
pub fn build_reg_write(id: u8, start_addr: u8, data: &[u8]) -> Vec<u8> {
    let mut params = vec![start_addr];
    params.extend_from_slice(data);
    build_packet(id, INST_REG_WRITE, &params)
}

/// 构建ACTION指令包 (广播)
pub fn build_action() -> Vec<u8> {
    build_packet(BROADCAST_ID, INST_ACTION, &[])
}

/// 构建RESET指令包
pub fn build_reset(id: u8) -> Vec<u8> {
    build_packet(id, INST_RESET, &[])
}

/// 解析应答包, 返回 (id, error, data)
pub fn parse_response(buf: &[u8]) -> Result<(u8, u8, Vec<u8>), String> {
    if buf.len() < 6 {
        return Err("应答包太短".to_string());
    }
    if buf[0] != 0xFF || buf[1] != 0xFF {
        return Err("应答包头错误".to_string());
    }
    let id = buf[2];
    let length = buf[3] as usize;
    if buf.len() < 4 + length {
        return Err(format!("应答包数据不完整, 期望 {} 字节, 实际 {} 字节", 4 + length, buf.len()));
    }
    let error = buf[4];
    let data = buf[5..3 + length].to_vec();
    let expected_checksum = calc_checksum(id, length as u8, error, &data);
    let actual_checksum = buf[3 + length];
    if expected_checksum != actual_checksum {
        return Err(format!("校验和错误: 期望 0x{:02X}, 实际 0x{:02X}", expected_checksum, actual_checksum));
    }
    Ok((id, error, data))
}

/// 寄存器信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterInfo {
    pub address: u8,
    pub name: String,
    pub bytes: u8,
    pub default_value: Option<i32>,
    pub storage: String,   // "EPROM" 或 "SRAM"
    pub permission: String, // "只读" 或 "读写"
    pub min_value: Option<i32>,
    pub max_value: Option<i32>,
    pub unit: String,
    pub description: String,
}

/// 获取所有寄存器定义
pub fn get_register_table() -> Vec<RegisterInfo> {
    vec![
        RegisterInfo { address: 0, name: "固件主版本号".into(), bytes: 1, default_value: Some(3), storage: "EPROM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "固件主版本号".into() },
        RegisterInfo { address: 1, name: "固件次版本号".into(), bytes: 1, default_value: Some(9), storage: "EPROM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "固件次版本号".into() },
        RegisterInfo { address: 3, name: "舵机主版本号".into(), bytes: 1, default_value: Some(9), storage: "EPROM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "舵机主版本号".into() },
        RegisterInfo { address: 5, name: "ID".into(), bytes: 1, default_value: Some(1), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(253), unit: "号".into(), description: "总线上唯一的身份识别码".into() },
        RegisterInfo { address: 6, name: "波特率".into(), bytes: 1, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(7), unit: "".into(), description: "0-7: 1000000,500000,250000,128000,115200,76800,57600,38400".into() },
        RegisterInfo { address: 7, name: "返回延时".into(), bytes: 1, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "2us".into(), description: "最大508us".into() },
        RegisterInfo { address: 8, name: "应答状态级别".into(), bytes: 1, default_value: Some(1), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(1), unit: "".into(), description: "0:部分应答 1:全应答".into() },
        RegisterInfo { address: 9, name: "最小角度限制".into(), bytes: 2, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(4094), unit: "步".into(), description: "运动行程最小值限制".into() },
        RegisterInfo { address: 11, name: "最大角度限制".into(), bytes: 2, default_value: Some(4095), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(1), max_value: Some(4095), unit: "步".into(), description: "运动行程最大值限制".into() },
        RegisterInfo { address: 13, name: "最高温度上限".into(), bytes: 1, default_value: Some(70), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(100), unit: "°C".into(), description: "最高工作温度限制".into() },
        RegisterInfo { address: 14, name: "最高输入电压".into(), bytes: 1, default_value: Some(80), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "0.1V".into(), description: "最高输入电压限制".into() },
        RegisterInfo { address: 15, name: "最低输入电压".into(), bytes: 1, default_value: Some(40), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "0.1V".into(), description: "最低输入电压限制".into() },
        RegisterInfo { address: 16, name: "最大扭矩".into(), bytes: 2, default_value: Some(1000), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(1000), unit: "‰".into(), description: "最大输出扭矩限制 1000=100%".into() },
        RegisterInfo { address: 18, name: "相位".into(), bytes: 1, default_value: Some(12), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "特殊功能字节".into() },
        RegisterInfo { address: 19, name: "卸载条件".into(), bytes: 1, default_value: Some(44), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "保护条件设置".into() },
        RegisterInfo { address: 20, name: "LED报警条件".into(), bytes: 1, default_value: Some(47), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "LED报警设置".into() },
        RegisterInfo { address: 21, name: "位置环P".into(), bytes: 1, default_value: Some(32), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "比例系数".into() },
        RegisterInfo { address: 22, name: "位置环D".into(), bytes: 1, default_value: Some(32), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "微分系数".into() },
        RegisterInfo { address: 23, name: "位置环I".into(), bytes: 1, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "积分系数".into() },
        RegisterInfo { address: 24, name: "最小启动力".into(), bytes: 1, default_value: Some(16), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "‰".into(), description: "最小输出启动扭矩".into() },
        RegisterInfo { address: 25, name: "积分限制值".into(), bytes: 1, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "0关闭积分限制".into() },
        RegisterInfo { address: 26, name: "顺时针不灵敏区".into(), bytes: 1, default_value: Some(1), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(32), unit: "步".into(), description: "".into() },
        RegisterInfo { address: 27, name: "逆时针不灵敏区".into(), bytes: 1, default_value: Some(1), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(32), unit: "步".into(), description: "".into() },
        RegisterInfo { address: 28, name: "保护电流".into(), bytes: 2, default_value: Some(500), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(511), unit: "6.5mA".into(), description: "最大3250mA".into() },
        RegisterInfo { address: 30, name: "角度分辨率".into(), bytes: 1, default_value: Some(1), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(1), max_value: Some(3), unit: "".into(), description: "控制圈数放大系数".into() },
        RegisterInfo { address: 31, name: "位置校正".into(), bytes: 2, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(-2047), max_value: Some(2047), unit: "步".into(), description: "BIT11为方向位".into() },
        RegisterInfo { address: 33, name: "运行模式".into(), bytes: 1, default_value: Some(0), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(3), unit: "".into(), description: "0:位置伺服 1:电机恒速 2:PWM开环 3:步进伺服".into() },
        RegisterInfo { address: 34, name: "保护扭矩".into(), bytes: 1, default_value: Some(20), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(100), unit: "%".into(), description: "过载保护后输出扭矩".into() },
        RegisterInfo { address: 35, name: "保护时间".into(), bytes: 1, default_value: Some(200), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "10ms".into(), description: "过载保护计时".into() },
        RegisterInfo { address: 36, name: "过载扭矩".into(), bytes: 1, default_value: Some(80), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(100), unit: "%".into(), description: "过载保护阈值".into() },
        RegisterInfo { address: 37, name: "速度闭环P".into(), bytes: 1, default_value: Some(10), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "".into(), description: "速度环比例系数".into() },
        RegisterInfo { address: 38, name: "过流保护时间".into(), bytes: 1, default_value: Some(200), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "10ms".into(), description: "最大2540ms".into() },
        RegisterInfo { address: 39, name: "速度闭环I".into(), bytes: 1, default_value: Some(200), storage: "EPROM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "1/10".into(), description: "速度环积分系数".into() },
        // SRAM区
        RegisterInfo { address: 40, name: "扭矩开关".into(), bytes: 1, default_value: Some(0), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(128), unit: "".into(), description: "0:关闭 1:打开 2:阻尼 128:校正为2048".into() },
        RegisterInfo { address: 41, name: "加速度".into(), bytes: 1, default_value: Some(0), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(254), unit: "100步/s²".into(), description: "加减速度".into() },
        RegisterInfo { address: 42, name: "目标位置".into(), bytes: 2, default_value: Some(0), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(-32766), max_value: Some(32766), unit: "步".into(), description: "绝对位置控制".into() },
        RegisterInfo { address: 44, name: "运行时间".into(), bytes: 2, default_value: Some(0), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(1000), unit: "‰".into(), description: "PWM模式用".into() },
        RegisterInfo { address: 46, name: "运行速度".into(), bytes: 2, default_value: Some(0), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(-32766), max_value: Some(32766), unit: "步/s".into(), description: "50步/s=0.732RPM".into() },
        RegisterInfo { address: 48, name: "转矩限制".into(), bytes: 2, default_value: Some(1000), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(1000), unit: "%".into(), description: "最大扭矩输出限制".into() },
        RegisterInfo { address: 55, name: "锁标志".into(), bytes: 1, default_value: Some(1), storage: "SRAM".into(), permission: "读写".into(), min_value: Some(0), max_value: Some(1), unit: "".into(), description: "0:EPROM可写 1:EPROM只读".into() },
        RegisterInfo { address: 56, name: "当前位置".into(), bytes: 2, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "步".into(), description: "当前位置反馈".into() },
        RegisterInfo { address: 58, name: "当前速度".into(), bytes: 2, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "步/s".into(), description: "当前速度反馈".into() },
        RegisterInfo { address: 60, name: "当前负载".into(), bytes: 2, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "‰".into(), description: "当前负载反馈".into() },
        RegisterInfo { address: 62, name: "当前电压".into(), bytes: 1, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "0.1V".into(), description: "当前电压反馈".into() },
        RegisterInfo { address: 63, name: "当前温度".into(), bytes: 1, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "°C".into(), description: "当前温度反馈".into() },
        RegisterInfo { address: 64, name: "异步写标志".into(), bytes: 1, default_value: Some(0), storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "异步写标志位".into() },
        RegisterInfo { address: 65, name: "舵机状态".into(), bytes: 1, default_value: Some(0), storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "错误状态".into() },
        RegisterInfo { address: 66, name: "移动标志".into(), bytes: 1, default_value: Some(0), storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "".into(), description: "1:运动中 0:停止".into() },
        RegisterInfo { address: 69, name: "当前电流".into(), bytes: 2, default_value: None, storage: "SRAM".into(), permission: "只读".into(), min_value: None, max_value: None, unit: "6.5mA".into(), description: "最大3250mA".into() },
    ]
}
