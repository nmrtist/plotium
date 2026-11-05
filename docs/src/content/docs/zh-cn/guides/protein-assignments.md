---
title: 人工蛋白峰指认
description: 将蛋白序列绑定到三维谱图并人工确认峰指认。
---

1. 选择 **Data → Import protein sequence** 并打开 FASTA 文件。每条记录成为同一个分子中的一条链。导入前核对分子名、链名和起始编号。项目保存序列，重开时无需原文件。
2. 选择一张三维谱，打开 **Analyze → Protein assignments**。选择分子并绑定当前谱图；为使用同一分子的其他谱图重复绑定。
3. 在三维峰表中拾取或选择一个峰。在 **Assignments** 中选择 **New candidate**。为每条采集轴（F1、F2、F3）选择一个残基，再选择一个原子或一组同元素的原子。允许留空。每种解释应单独保存为一个候选。
4. 点击 **Confirm** 接受一个候选。部分指认仍保留未指认轴；原子组确认前须选择 **One of these atoms (identity unresolved)**（其中一个原子，身份未定）或 **All members share this resonance**（所有成员共享该共振）。**Unspecified (candidate only)**（含义未定，仅候选）允许保存，但不能确认。共享共振不表示化学等价或立体专一指认。Plotium 按本地 [wwPDB 化学组分字典](https://www.wwpdb.org/data/ccd) 校验原子名。确认前，每条已指认轴都必须有已知核种。
5. 展开 **Sequence and associated peaks**，选择残基后点击结果，可切换到对应谱图并定位峰。列表同时显示候选与已确认结果。

编辑器按轴核种和标准线性肽链中的位置提供原子：`H2` 仅用于首个残基，`OXT`/`HXT` 仅用于末个残基，Pro 的 `H` 仅用于首个残基。检查依据序列位置，而非显示编号；自动化和项目验证使用相同规则。目录不推断末端质子化、封端、二硫键或其他修饰。**Confirmed** 表示人工接受，不表示已自动验证化学合理性或实验连接关系。

修改已确认候选的原子目标或原子组含义会撤回确认。修改候选备注、移动峰、切换切片、调整相位或重新编号均保留确认。删除峰会一并删除其指认，撤销会一起恢复。更改绑定前先清除该谱的指认。清除前会显示谱名、受影响峰数、候选数和确认数，供确认或取消。即使选择另一张谱，清除仍针对最初的谱；若项目发生变化，须重新核对数量再确认。删除分子前先解除其所有谱图绑定。

保存项目以保留序列、绑定和指认。在 **Molecule details** 中选择 **Export JSON**，可导出候选、确认状态、坐标、带符号的峰高、核种和处理来源。选中 **Export only confirmed assignments** 可省略其他候选。报告保留每条独立观测。

面板内容超出可用高度时可滚动，向下滚动即可到达 **Save candidate**。**Active spectrum** 显示谱名，**Bound molecule** 显示绑定分子。序列浏览器可查看其他分子；候选编辑器会单独显示其目标谱和分子。切换峰或谱时，已有草稿仍保留原目标。

自动化接口使用文档目标。CLI 可通过 `plotium-cli tool run` 调用 `protein.edit` 导入 FASTA 文本。用 `protein.assignments.edit` 和明确的谱及峰 ID 编辑指认，用 `protein.inspect` 查询，用 `protein.assignments.export` 导出。MCP 客户端通过 `plotium_plan` 和 `plotium_execute` 执行编辑；读取使用 `plotium_protein_inspect` 和 `plotium_protein_assignments_export`。

查询默认返回精简的分子摘要、谱图绑定和关联峰。读取序列时，先用 `view: "chains"` 和分子 ID 查询链，再用 `view: "residues"`、分子 ID 和链 ID 查询残基。每条残基记录包含稳定 ID、单字母代码和显示编号。用 `limit` 控制页大小，并按 `next_offset` 翻页，直到其为 null。JSON 导出包含完整序列。

每个原子目标包含 `residue`、`members` 和 `group_meaning`。后者默认为 `unspecified`；`one_of` 表示其中一个原子、身份未定，`shared` 表示所有成员共享该观测共振。单个原子使用 `unspecified`。改变该值与改变原子一样会撤回确认。JSON 导出包含此字段及 `assignment_semantics` 解释；不会自动合并或平均观测。例如丙氨酸甲基共振可使用 `members: ["HB1", "HB2", "HB3"]` 和 `group_meaning: "shared"`。

## 科学上下文与首版结构流程范围

在蛋白指认面板展开 **Scientific context**，点击 **Edit sample conditions and experiments**，打开针对开始编辑时活动谱图的表单。选择已有样品条件，或点击 **New sample conditions** 新建，ID 自动维护。记录开尔文温度、pH（含边界 0–14）、溶剂、同位素标记和构建体；取消温度或 pH 的勾选可保留为未知。在 **Nonstandard chemistry** 中注明修饰、配体或特殊成键，仅在审阅后确认 **Reviewed: single major conformer**。选择实验类型，按采集 F1/F2/F3 顺序选择轴角色，并记录元数据来源。修改共享样品会影响所有关联谱图；不同条件应新建独立记录。保存会校验草稿，支持撤销/重做，并随项目内嵌保存。

草稿保留原始谱图及上下文基线。如果撤销/重做或其他操作改变了当前上下文，保存将被阻止。点击 **Reload current context (discard draft)** 用当前记录替换草稿，或点击 **Discard context edits** 放弃草稿。无关文档编辑不会使基线失效。草稿目标谱图被删除时，即使该谱图尚未保存实验元数据，也会阻止保存。重新加载后仅编辑样品；点击 **Keep sample edits (sample-only)** 可移除已删除谱图的关联并保留样品编辑。需先应用或取消高级 JSON 编辑；若已保存上下文也发生变化，仍需解决该冲突。可选的 **Advanced JSON** 高级入口需先应用或取消 JSON 编辑才能保存；应用 JSON 只更新草稿，不绕过校验或冲突保护。

实验通过稳定的样品条件 ID 关联样品。仅在条件完全相同时复用 ID，不同条件使用不同记录。角色按采集轴 F1/F2/F3 排列，独立于显示顺序和采集核种。`observed_proton` 与 `indirect_proton` 区分 NOESY 的两个质子轴。未知实验和角色仍可浏览及指认，但不能进入依赖角色的结构操作；已知角色必须匹配采集核种。

首版结构流程限定单链标准残基、单一主要构象和受支持的稠密 3D 谱，采用人工指认与外部计算。范围检查报告缺失上下文及不支持的化学体系，不认证实验连通性、指认或 NOE 测量；本步骤尚未提供结构计算或约束生成。

## 化学位移参考

在蛋白指认面板展开 **Chemical-shift reference**，点击 **Edit reference**。按采集顺序填写 F1/F2/F3 的绝对偏移量（ppm），记录参考标准或比较证据，并手工确认审阅后保存。校正坐标等于采集坐标加偏移量；重复保存同一偏移量不会重复叠加。谱图切片、峰坐标和 JSON 导出均使用校正后的 ppm。峰 ID 和指认保持稳定，撤销/重做及项目保存保留参考及来源，相位变化也会保留校正。

面板显示与其他已记录实验元数据的谱图进行参考比较的准备状态。双方必须手工确认参考，并关联同一个样品条件 ID；不同条件不能静默合并。这是人工参考审阅，不是自动峰对齐，也不认证核种、轴角色或化学位移一致性。合并观测前仍需检查这些信息。参考变更可能使下游科学解释失效。

对原子身份和核种相同的已确认单原子观测，逐条显示左谱减右谱的校正后 ppm 差值。排除歧义原子组；没有匹配项时显示未评估。不做平均、容差判定或自动校正。

参考表单显示草稿目标谱名。切换谱图后会提示：保存仍修改该草稿目标。参考坐标变化（包括撤销/重做）会清除受影响的 Inspect/Delta 固定游标及切片游标，因为原有读数已失效。
