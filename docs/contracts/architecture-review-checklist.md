---
summary: "Cross-contract checklist for reviewing architecture proposals and code changes."
read_when:
  - Reviewing a design proposal that crosses module or authority boundaries
  - Reviewing code that changes data flow, state ownership, lifecycle, or fallbacks
---

# Architecture Review Checklist

用于方案评审和代码评审。只检查与当前变更相关的条目；方案应说明如何满足，
代码应提供可复核的证据。

## 1. 职责与边界

- [ ] 变更的目标、责任边界和受影响的数据流是否明确？
- [ ] 每个模块是否只有一个主要职责和一类稳定的变更原因？
- [ ] 展示、协调、平台能力、文档计算、持久化和协议定义是否彼此分离？
- [ ] 现有职责是否被明确保留、转移或删除，而不是被重复实现？
- [ ] 新增抽象是否确实减少复杂度，而不是增加 wrapper、shim、兼容别名或第二条路径？

## 2. 依赖方向与契约

- [ ] 依赖是否沿分层方向流动，没有反向依赖或循环依赖？
- [ ] 浏览器、桌面和扩展能力是否通过平台边界进入共享逻辑？
- [ ] 文档计算是否不依赖 UI、浏览器或桌面实现？
- [ ] 公共契约是否只暴露客户端需要的数据，不泄露服务端内部模型？
- [ ] 协议、序列化资源、API schema 和规范化投影是否各自只有一个真源，并在边界校验？
- [ ] 消费者是否复用规范契约，而不是重新解释或维护弱化版本？

## 3. Authority、状态所有权与数据流

- [ ] 每类状态是否只有一个可写 authority；镜像、缓存、投影和持久化数据是否只是派生状态？
- [ ] 草稿文本、文档语义、快照、工作区拓扑、导航路径、预览和 session 的 owner 是否清楚？
- [ ] 是否能用以下形式描述端到端数据流：
  `输入 → 规划/规范化 → 唯一写入或运行时 → 权威结果 → 绑定/投影 → 视图`？
- [ ] 所有主文档写入是否经过唯一的 canonical commit path？不同入口是否最终汇入同一条路径？
- [ ] 图编辑是否先经过规划；语义读取是否绑定目标文档和语义版本，且不静默回退到其他或最新结果？
- [ ] 视图、协调层、局部投影和临时分析是否只消费权威结果，不重新定义语义或成为第二 authority？
- [ ] 持久化是否只从 live authority 单向投影；共享、侧车和辅助状态是否不会越权进入主文档或主 Tab？
- [ ] 导航路径及其派生表面是否由单一 authority 驱动，局部编辑完成是否以主文档事务终态为准？

## 4. 异步、并发与生命周期

- [ ] 异步操作是否在第一次让出执行前捕获完整稳定目标？
- [ ] 完成时是否校验捕获目标，而不是重新读取当时的 active 状态？
- [ ] 是否区分 document-current 与 visible-current，避免后台或旧结果污染当前视图？
- [ ] 同一 owner/scope 的新操作是否按定义使旧操作失效，同时不影响独立操作？
- [ ] 取消、重试、dispose、资源释放和 stale cleanup 是否各有唯一 owner，且幂等？
- [ ] 成功、失败、取消、替换、关闭和 dispose 后是否都不会留下迟到写入或未释放资源？

## 5. 失败状态与 fallback

- [ ] ready、snapshot-not-ready/loading、empty/clear、invalid、parse failure、cancelled、stale、rejected、no-op 和 protocol failure 是否按契约区分？
- [ ] fallback 是否是明确的产品决策，具备触发条件、可见结果、原因和必要的可观测性？长期兼容 fallback 是否有复查或退出条件？
- [ ] 是否禁止用空值掩盖 not-ready、无效响应或失败，或用旧语义伪装当前成功？
- [ ] 失败是否由所属边界处理，并按契约保留或清除 authority，而不是静默 no-op？
- [ ] restore、promotion 等状态机流程是否明确触发条件、single-flight、发布与持久化顺序、失败和重试语义？

## 6. 跨路径一致性与安全

- [ ] full build、incremental、import、replacement 和 streaming 在代表同一操作时是否收敛到相同语义？
- [ ] streaming 是否区分真实中间语义与仅传输分块，并明确 close finalization 与最终快照的边界？
- [ ] 布局、虚拟化和渲染投影是否只改变呈现/物化，不改变拓扑、路径、锚点或语义身份？
- [ ] Web、Desktop、Extension、Graph、Column、Preview 和 Sharing 是否复用相同的语义与契约边界？
- [ ] 平台权限是否遵循最小授权；用户内容、凭据、路径和其他隐私数据是否留在规定边界内？

## 7. 验证证据

- [ ] 评审材料是否说明了该路径的 authority、依赖方向、数据流、失败行为和生命周期？
- [ ] 是否有边界级测试、契约检查或其他证据，而不只是内部实现分支覆盖？
- [ ] 是否覆盖适用的负向路径：stale、取消、关闭/dispose、not-ready、parse failure、blank/clear、无效契约、重试和并发？
- [ ] 是否验证等价执行路径的收敛，以及局部更新不会无故改变无关结果？
- [ ] 生成的协议、schema 和投影是否与真源同步并经过校验？
- [ ] 是否运行并记录了与变更边界匹配的最小静态、单元、集成或端到端检查？
- [ ] 无法满足的适用条目是否有明确、有限且记录在案的例外理由？

## Review result

- [ ] 所有适用条目都有证据。
- [ ] 没有引入新的 authority、依赖捷径、重复路径、静默 fallback 或未记录的兼容行为。
- [ ] 剩余风险和后续工作有负责人及明确的关闭条件。
