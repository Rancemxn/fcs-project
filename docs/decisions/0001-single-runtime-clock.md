# 0001：运行时只有一个物理主时钟

状态：Accepted

## 决策

FCS Core 运行时只使用绝对物理时间 `chartTime`。全局 `chartBeat` 是由 tempo map 与
`chartTime` 双向映射的音乐坐标；每条 Line 的 `lineScrollCoordinate` 是 `chartTime` 的函数。
Beat 和滚动坐标都不是可独立暂停、快进、倒放或推进的物理时钟。

## 理由

判定、音频、Hold、Line motion、scroll 和 Render 如果分别推进，会因 frame rate、seek、暂停
或来源格式的 line BPM 产生漂移。单一物理时钟使 seek 可以直接求值，也让跨格式 Note 判定
有共同基准。

## 后果

- Note 最终判定时间必须归一化为 chartTime；
- RPE bpmfactor 和 PGR line BPM 不能隐式改变判定时间；
- runtime 不维护 line-local clock state；
- scroll 和 floor distance 必须可由 chartTime 查询。
