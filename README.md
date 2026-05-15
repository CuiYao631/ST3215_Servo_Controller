# ST3215 舵机控制器

基于 Tauri v2 开发的 ST3215 舵机调试工具，通过串口（RS485）与舵机通讯，支持查看/修改舵机参数、控制舵机旋转。

## 功能

- **串口连接** — 自动检测可用串口，波特率 1Mbps
- **舵机扫描** — Ping 指定 ID 的舵机
- **状态读取** — 实时读取位置、速度、负载、温度、电压
- **运动控制** — 设置目标位置、速度、加速度，控制舵机旋转
- **扭矩开关** — 启用/禁用舵机扭矩输出
- **寄存器读写** — 完整的 EPROM/SRAM 寄存器表，支持单个或批量读取、写入

## 技术栈

- **前端** — Vanilla JS + Vite
- **后端** — Rust + Tauri v2
- **串口** — serialport crate（RS485 协议）
- **舵机协议** — 飞特 ST3215，12 位磁编码器（4096 步）

## 开发

```bash
# 安装依赖
npm install

# 启动开发模式
cargo tauri dev

# 构建发布版本
cargo tauri build
```

## 项目结构

```
├── src/
│   ├── main.js          # 前端逻辑
│   └── style.css        # 样式
├── src-tauri/
│   └── src/
│       ├── lib.rs              # Tauri 命令（串口管理、舵机操作）
│       ├── servo_protocol.rs   # ST3215 协议实现
│       └── main.rs             # 入口
├── index.html           # 页面布局
└── vite.config.js
```

## 协议说明

ST3215 使用半双工 RS485 通讯，数据包格式：

```
[0xFF] [0xFF] [ID] [Length] [Instruction] [Param...] [Checksum]
```

Checksum = ~(ID + Length + Instruction + Params) & 0xFF，多字节数据低位在前。
