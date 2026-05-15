const { invoke } = window.__TAURI__.core;

// ====== 带超时的 invoke ======
function invokeWithTimeout(cmd, args = {}, timeoutMs = 5000) {
  return Promise.race([
    invoke(cmd, args),
    new Promise((_, reject) =>
      setTimeout(() => reject(`命令 ${cmd} 超时 (${timeoutMs}ms)`), timeoutMs)
    ),
  ]);
}

// ====== 日志 ======
function log(msg, type = "info") {
  const el = document.getElementById("log-content");
  const time = new Date().toLocaleTimeString("zh-CN", { hour12: false });
  const div = document.createElement("div");
  div.className = `log-entry ${type}`;
  div.innerHTML = `<span class="time">[${time}]</span> ${msg}`;
  el.appendChild(div);
  el.scrollTop = el.scrollHeight;
}

// ====== 状态 ======
let connected = false;
let autoRefreshTimer = null;
let registerDefs = [];
let registerValues = {};

// ====== 串口管理 ======
async function refreshPorts() {
  try {
    const ports = await invokeWithTimeout("list_ports", {}, 3000);
    const select = document.getElementById("port-select");
    const current = select.value;
    select.innerHTML = '<option value="">选择串口...</option>';
    ports.forEach((p) => {
      const opt = document.createElement("option");
      opt.value = p.name;
      opt.textContent = `${p.name} (${p.port_type})`;
      select.appendChild(opt);
    });
    if (current) select.value = current;
    log(`发现 ${ports.length} 个串口`);
  } catch (e) {
    log(`刷新串口失败: ${e}`, "error");
  }
}

async function toggleConnection() {
  if (connected) {
    try {
      await invokeWithTimeout("disconnect_port", {}, 3000);
      connected = false;
      updateConnectionUI();
      log("已断开连接");
    } catch (e) {
      log(`断开失败: ${e}`, "error");
    }
  } else {
    const portName = document.getElementById("port-select").value;
    const baudRate = parseInt(document.getElementById("baud-rate-select").value);
    if (!portName) {
      log("请先选择串口", "error");
      return;
    }
    try {
      await invokeWithTimeout("connect_port", { portName, baudRate }, 5000);
      connected = true;
      updateConnectionUI();
      log(`已连接 ${portName} @ ${baudRate}`, "receive");
    } catch (e) {
      log(`连接失败: ${e}`, "error");
    }
  }
}

function updateConnectionUI() {
  const btn = document.getElementById("connect-btn");
  const status = document.getElementById("connection-status");
  if (connected) {
    btn.textContent = "断开";
    btn.className = "btn-danger";
    status.textContent = "已连接";
    status.className = "status-connected";
  } else {
    btn.textContent = "连接";
    btn.className = "";
    status.textContent = "未连接";
    status.className = "status-disconnected";
  }
}

// ====== 舵机操作 ======
function getServoId() {
  return parseInt(document.getElementById("servo-id").value) || 1;
}

async function pingServo() {
  if (!connected) { log("请先连接串口", "error"); return; }
  try {
    const id = getServoId();
    log(`PING 舵机 ID=${id}...`, "send");
    const error = await invokeWithTimeout("ping_servo", { id }, 3000);
    log(`PING 成功, 状态: 0x${error.toString(16).padStart(2, "0")}`, "receive");
  } catch (e) {
    log(`PING 失败: ${e}`, "error");
  }
}

async function readServoStatus() {
  if (!connected) return;
  try {
    const id = getServoId();
    const status = await invokeWithTimeout("read_servo_status", { id }, 2000);
    document.getElementById("status-position").textContent = status.position;
    document.getElementById("status-speed").textContent = status.speed;
    document.getElementById("status-load").textContent = status.load;
    document.getElementById("status-voltage").textContent = status.voltage.toFixed(1);
    document.getElementById("status-temp").textContent = status.temperature;
    document.getElementById("status-current").textContent = status.current.toFixed(1);
    document.getElementById("status-moving").textContent = status.moving ? "运动中" : "停止";
    document.getElementById("status-moving").style.color = status.moving ? "#f39c12" : "#2ecc71";
    document.getElementById("status-error").textContent = `0x${status.error.toString(16).padStart(2, "0")}`;
    document.getElementById("status-error").style.color = status.error ? "#e74c3c" : "#2ecc71";
  } catch (e) {
    log(`读取状态失败: ${e}`, "error");
  }
}

async function moveServo() {
  if (!connected) { log("请先连接串口", "error"); return; }
  try {
    const id = getServoId();
    const position = parseInt(document.getElementById("target-position").value);
    const speed = parseInt(document.getElementById("target-speed").value);
    const acceleration = parseInt(document.getElementById("target-acc").value);
    log(`移动舵机 ID=${id} → 位置=${position} 速度=${speed} 加速度=${acceleration}`, "send");
    await invokeWithTimeout("move_servo", { id, position, speed, acceleration }, 3000);
    log("移动指令已发送", "receive");
  } catch (e) {
    log(`移动失败: ${e}`, "error");
  }
}

async function setTorque(value) {
  if (!connected) { log("请先连接串口", "error"); return; }
  try {
    const id = getServoId();
    const names = { 0: "关闭", 1: "打开", 2: "阻尼" };
    log(`设置扭矩: ${names[value] || value}`, "send");
    await invokeWithTimeout("set_torque", { id, enable: value }, 3000);
    log("扭矩设置成功", "receive");
  } catch (e) {
    log(`设置扭矩失败: ${e}`, "error");
  }
}

// ====== 寄存器管理 ======
async function loadRegisterDefs() {
  registerDefs = await invoke("get_register_info");
  renderRegisterTable();
}

function renderRegisterTable() {
  const filter = document.getElementById("register-filter-select").value;
  const tbody = document.getElementById("register-tbody");
  tbody.innerHTML = "";

  const filtered = registerDefs.filter((reg) => {
    if (filter === "EPROM") return reg.storage === "EPROM";
    if (filter === "SRAM") return reg.storage === "SRAM";
    if (filter === "rw") return reg.permission === "读写";
    if (filter === "ro") return reg.permission === "只读";
    return true;
  });

  filtered.forEach((reg) => {
    const tr = document.createElement("tr");
    const val = registerValues[reg.address];
    const valStr = val !== undefined ? val : "--";
    const rangeStr =
      reg.min_value !== null && reg.max_value !== null
        ? `${reg.min_value}~${reg.max_value}`
        : "--";

    const isWritable = reg.permission === "读写";
    const storageClass = reg.storage === "EPROM" ? "eprom" : "sram";

    tr.innerHTML = `
      <td class="addr">0x${reg.address.toString(16).padStart(2, "0").toUpperCase()}</td>
      <td>${reg.name}</td>
      <td>${reg.bytes}</td>
      <td class="value-cell" id="reg-val-${reg.address}">${valStr}</td>
      <td>${reg.default_value !== null ? reg.default_value : "--"}</td>
      <td>${rangeStr}</td>
      <td>${reg.unit}</td>
      <td class="${storageClass}">${reg.storage}</td>
      <td class="${isWritable ? "" : "readonly"}">${reg.permission}</td>
      <td>
        <button class="reg-read-btn" data-addr="${reg.address}" data-bytes="${reg.bytes}">读</button>
        ${isWritable ? `<input type="number" class="reg-write-input" id="reg-input-${reg.address}" placeholder="值" />
        <button class="reg-write-btn" data-addr="${reg.address}" data-bytes="${reg.bytes}">写</button>` : ""}
      </td>
    `;
    tbody.appendChild(tr);
  });

  // 绑定事件
  tbody.querySelectorAll(".reg-read-btn").forEach((btn) => {
    btn.addEventListener("click", () => readSingleRegister(parseInt(btn.dataset.addr), parseInt(btn.dataset.bytes)));
  });
  tbody.querySelectorAll(".reg-write-btn").forEach((btn) => {
    btn.addEventListener("click", () => writeSingleRegister(parseInt(btn.dataset.addr), parseInt(btn.dataset.bytes)));
  });
}

async function readSingleRegister(addr, bytes) {
  if (!connected) { log("请先连接串口", "error"); return; }
  try {
    const id = getServoId();
    const data = await invokeWithTimeout("read_register", { id, addr, len: bytes }, 2000);
    let value;
    if (bytes === 1) {
      value = data[0];
    } else {
      value = data[0] | (data[1] << 8);
      // 处理有符号数
      if (value > 32767) value -= 65536;
    }
    registerValues[addr] = value;
    const cell = document.getElementById(`reg-val-${addr}`);
    if (cell) cell.textContent = value;
    log(`读取 0x${addr.toString(16).padStart(2, "0")}: ${value} [${data.map(b => "0x" + b.toString(16).padStart(2, "0")).join(" ")}]`, "receive");
  } catch (e) {
    log(`读取 0x${addr.toString(16).padStart(2, "0")} 失败: ${e}`, "error");
  }
}

async function writeSingleRegister(addr, bytes) {
  if (!connected) { log("请先连接串口", "error"); return; }
  const input = document.getElementById(`reg-input-${addr}`);
  if (!input || input.value === "") { log("请输入要写入的值", "error"); return; }

  const value = parseInt(input.value);
  let data;
  if (bytes === 1) {
    data = [value & 0xff];
  } else {
    data = [value & 0xff, (value >> 8) & 0xff];
  }

  try {
    const id = getServoId();
    log(`写入 0x${addr.toString(16).padStart(2, "0")}: ${value} [${data.map(b => "0x" + b.toString(16).padStart(2, "0")).join(" ")}]`, "send");
    await invokeWithTimeout("write_register", { id, addr, data }, 2000);
    log("写入成功", "receive");
    // 写入后重新读取验证
    await readSingleRegister(addr, bytes);
  } catch (e) {
    log(`写入 0x${addr.toString(16).padStart(2, "0")} 失败: ${e}`, "error");
  }
}

async function readAllRegisters() {
  if (!connected) { log("请先连接串口", "error"); return; }
  try {
    const id = getServoId();
    log(`读取舵机 ID=${id} 全部寄存器...`, "send");
    const results = await invokeWithTimeout("read_all_registers", { id }, 30000);
    results.forEach(([addr, data]) => {
      if (data.length > 0) {
        const reg = registerDefs.find((r) => r.address === addr);
        let value;
        if (reg && reg.bytes === 1) {
          value = data[0];
        } else if (data.length >= 2) {
          value = data[0] | (data[1] << 8);
          if (value > 32767) value -= 65536;
        } else {
          value = data[0] || 0;
        }
        registerValues[addr] = value;
      }
    });
    renderRegisterTable();
    log(`读取完成, 共 ${results.length} 个寄存器`, "receive");
  } catch (e) {
    log(`读取全部失败: ${e}`, "error");
  }
}

// ====== 滑块同步 ======
function syncSlider(sliderId, inputId) {
  const slider = document.getElementById(sliderId);
  const input = document.getElementById(inputId);
  slider.addEventListener("input", () => (input.value = slider.value));
  input.addEventListener("input", () => (slider.value = input.value));
}

// ====== 初始化 ======
document.addEventListener("DOMContentLoaded", async () => {
  // 绑定事件
  document.getElementById("refresh-ports-btn").addEventListener("click", refreshPorts);
  document.getElementById("connect-btn").addEventListener("click", toggleConnection);
  document.getElementById("ping-btn").addEventListener("click", pingServo);
  document.getElementById("read-all-btn").addEventListener("click", readAllRegisters);
  document.getElementById("refresh-status-btn").addEventListener("click", readServoStatus);
  document.getElementById("move-btn").addEventListener("click", moveServo);
  document.getElementById("torque-on-btn").addEventListener("click", () => setTorque(1));
  document.getElementById("torque-off-btn").addEventListener("click", () => setTorque(0));
  document.getElementById("torque-damping-btn").addEventListener("click", () => setTorque(2));
  document.getElementById("clear-log-btn").addEventListener("click", () => {
    document.getElementById("log-content").innerHTML = "";
  });
  document.getElementById("register-filter-select").addEventListener("change", renderRegisterTable);

  // 自动刷新
  document.getElementById("auto-refresh").addEventListener("change", (e) => {
    if (e.target.checked) {
      autoRefreshTimer = setInterval(readServoStatus, 200);
      log("自动刷新已开启 (200ms)");
    } else {
      clearInterval(autoRefreshTimer);
      autoRefreshTimer = null;
      log("自动刷新已关闭");
    }
  });

  // 快捷位置按钮
  document.querySelectorAll(".quick-pos-btn").forEach((btn) => {
    btn.addEventListener("click", () => {
      const pos = parseInt(btn.dataset.pos);
      document.getElementById("target-position").value = pos;
      document.getElementById("target-position-slider").value = pos;
    });
  });

  // 滑块同步
  syncSlider("target-position-slider", "target-position");
  syncSlider("target-speed-slider", "target-speed");
  syncSlider("target-acc-slider", "target-acc");

  // 加载寄存器定义和串口列表
  await loadRegisterDefs();
  await refreshPorts();

  log("ST3215 舵机控制器已启动", "receive");
});
