
# K3D Scaffold Plugin（圓盤式施工架）設計文件  
**Version: v0.1**

---

## 1. 產品定位

本外掛目標：

👉 將建築空間自動轉換為「圓盤式施工架配置方案」

---

## 2. 解決問題

- 手動排架效率低
- 材料難估算
- 安全規範難落實

---

## 3. 系統架構

```text
K3D Core
  ├─ Geometry / Inference
  └─ Plugin API

Scaffold Plugin
  ├─ 空間解析
  ├─ 排架引擎
  ├─ 規則檢核
  ├─ BOM計算
  └─ 圖面輸出
```

---

## 4. 模組設計

### 4.1 space_analyzer.py

```python
def analyze_region(selection):
    bbox = geom.get_bbox(selection)
    return {
        "width": bbox.width,
        "height": bbox.height,
        "depth": bbox.depth
    }
```

---

### 4.2 generator.py（核心）

```python
def generate_scaffold(region):
    spacing = 1500
    levels = int(region["height"] / 1500)
    bays_x = int(region["width"] / spacing)
    bays_y = int(region["depth"] / spacing)

    return {
        "standards": [],
        "ledgers": [],
        "diagonals": []
    }
```

---

### 4.3 system_library.py

```python
RINGLOCK = {
    "standard": [1000,1500,2000],
    "ledger": [1200,1500,1800]
}
```

---

### 4.4 safety_engine.py

```python
def check_safety(scaffold):
    issues = []

    if scaffold["height"] > 2000:
        issues.append("需要斜撐")

    return issues
```

---

### 4.5 bom.py

```python
def calculate_bom(scaffold):
    return {
        "standards": len(scaffold["standards"]),
        "ledgers": len(scaffold["ledgers"]),
        "diagonals": len(scaffold["diagonals"])
    }
```

---

### 4.6 exporter.py

```python
def export_layout(data):
    pass
```

---

## 5. 使用流程

```text
1. 選取區域
2. 啟動 Scaffold Plugin
3. 自動排架
4. 檢核安全
5. 輸出 BOM / 圖面
```

---

## 6. UI建議

- 間距設定
- 層高設定
- 安全檢核開關
- 一鍵生成

---

## 7. MVP建議

1. 固定間距排架
2. 基本BOM
3. 簡單安全檢查

---

## 8. 未來擴展

- 法規檢核
- 多廠牌系統
- 自動報價
- 與機器人整合

---

## 9. 結論

👉 Scaffold Plugin 是最容易快速落地的模組  
👉 可作為 K3D 第一個商業成功場景

