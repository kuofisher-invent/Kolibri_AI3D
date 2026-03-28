# Kolibri Ai3D — 預判式互動系統升級設計（超越 SketchUp 體感）

## 🎯 目標
建立預判式互動系統，讓滑鼠移動即回饋、減少工具切換並預測使用者意圖。

## 核心概念
滑鼠位置 + 幾何上下文 + 工具狀態 + 歷史行為 = 使用者意圖預測

## 系統流程
Mouse Move → Candidate 收集 → Context 建立 → Scoring → Preview → User Action → Logging

## Candidate 類型
- 幾何：端點、中點、交點、邊、面、軸
- 操作：畫線、延伸、移動、複製、推拉

## Interaction Context
- tool
- hover type
- selection
- modifier
- cursor velocity / dwell
- last actions

## Scoring
Score = 幾何 + 工具 + 上下文 + 歷史 + 意圖

## Preview
- 主候選（高亮 + ghost）
- 次候選（淡顯）
- 提示文字（Endpoint / Axis / Face）

## Ghost
- ghost line / face / extrusion

## 歷史
記錄最近 1~3 步操作

## Training Data
記錄 input + 最終選擇

## 發展階段
Phase1：規則引擎
Phase2：使用者偏好
Phase3：意圖模型

## 優先工具
- Line
- Move
- Push/Pull

## 成功指標
- 100ms 內回饋
- undo 減少
- 建模更快
