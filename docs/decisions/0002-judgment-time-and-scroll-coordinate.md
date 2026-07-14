# 0002：分离判定时间与滚动坐标

状态：Accepted

## 决策

Gameplay 判定使用 Note 的 canonical chartTime。Line 滚动使用 lineScrollCoordinate、
scrollSpeed 和 scrollDistance。两者共享同一物理时钟，但在语义和数据模型中分离。

## 理由

外部格式常把 BPM、tick、speed 和 floorPosition 混合。若 FCS 延续这种混合，就无法判断一个
字段改变的是音乐时刻还是视觉滚动，也无法给出可靠的转换损失报告。

## 后果

- `scrollTempoMap` 不能成为第二判定轴；
- `scrollSpeed` 与 Note `scrollFactor` 使用不同名称；
- Converter 必须分别报告 time quantization 和 distance approximation；
- 可视位置变化不得反向改变 gameplay time。
