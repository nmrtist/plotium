//! Discovery schemas shared by every scientific frontend.
use serde_json::{Value, json};
fn external(name: &str, value: Value) -> Value {
    json!({"type":"object","required":[name],"additionalProperties":false,"properties":{name:value}})
}
fn object(fields: Value) -> Value {
    let required = fields
        .as_object()
        .expect("literal schema properties")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    json!({"type":"object","required":required,"additionalProperties":false,"properties":fields})
}
pub(super) fn step_kind() -> Value {
    let number = json!({"type":"number"});
    let window = json!({"type":"integer","minimum":1,"maximum":65535});
    json!({"oneOf":[
        {"enum":["Fft","Magnitude","Reverse","Invert"]},
        external("ZeroFill",json!({"type":"string","pattern":"^(none|factor[0-9]+|size[0-9]+)$"})),
        external("Apodize",json!({"oneOf":[{"enum":["None","CosineBell"]},external("Exponential",object(json!({"lb_hz":number}))),external("Gaussian",object(json!({"lb_hz":number,"gb_hz":number})))]})),
        external("Phase",object(json!({"phase0":number,"phase1":number,"pivot_frac":number,"auto":{"type":["string","null"],"enum":[null,"RobustConsensus","AbsorptivePeak","Entropy","NegativeMinimization","PeakRegression"]}}))),
        external("Baseline",json!({"oneOf":[{"const":"Offset"},external("Polynomial",object(json!({"order":{"type":"integer","minimum":0,"maximum":255}}))),external("AsymmetricLeastSquares",object(json!({"smoothness":number,"asymmetry":number,"iterations":window})))]})),
        external("Reference",object(json!({"at_ppm":number,"target_ppm":number}))),
        external("Smooth",json!({"oneOf":[external("MovingAverage",object(json!({"window":window}))),external("SavitzkyGolay",object(json!({"window":window,"poly_order":{"type":"integer","minimum":0,"maximum":255}})))]})),
        external("Normalize",json!({"oneOf":[{"enum":["MaxPeak","TotalArea"]},external("Constant",object(json!({"divisor":number})))]})),
        external("Bin",object(json!({"width":number,"method":{"enum":["Sum","Mean"]}}))),
        external("Window",object(json!({"low_ev":number,"high_ev":number})))
    ]})
}
pub(super) fn xrd_processing() -> Value {
    let background = json!({"oneOf":[{"type":"null"},object(json!({"iterations":{"type":"integer","minimum":1,"maximum":200}}))]});
    let smoothing = json!({"oneOf":[{"type":"null"},object(json!({"window":{"type":"integer","minimum":3,"maximum":101},"polynomial_order":{"type":"integer","minimum":0,"maximum":6}}))]});
    object(
        json!({"background":background,"smoothing":smoothing,"normalization":{"enum":["None","Maximum","Area"]}}),
    )
}
pub(super) fn fit_options() -> Value {
    object(json!({
        "solver":{"enum":["bounded_trust_region","levenberg_marquardt"]},
        "weights":{"oneOf":[object(json!({"type":{"enum":["auto","equal","measurement_sigma","relative","poisson"]}})),object(json!({"type":{"const":"expression"},"expression":{"type":"string"}}))]},
        "robust_loss":{"oneOf":[object(json!({"type":{"const":"none"}})),object(json!({"type":{"enum":["huber","soft_l1","cauchy"]},"scale":{"type":"number","exclusiveMinimum":0}}))]},
        "non_finite":{"enum":["reject","exclude_rows"]},
        "max_iterations":{"type":"integer","minimum":1,"maximum":10000},
        "tolerance":{"type":"number","exclusiveMinimum":0},
        "multi_start":{"type":"integer","minimum":1,"maximum":100},
        "ode_solver":{"enum":["auto","dormand_prince45","bdf"]},
        "relative_tolerance":{"type":"number","exclusiveMinimum":0},
        "absolute_tolerance":{"type":"number","exclusiveMinimum":0}
    }))
}
pub(super) fn task_result() -> Value {
    json!({"type":"object","required":["tool_id","before_revision","after_revision","targets","value"],"properties":{
        "tool_id":{"type":"string"},"before_revision":{"type":"integer"},"after_revision":{"type":"integer"},"targets":{"type":"array"},
        "produced":{"type":"array"},"modified":{"type":"array"},
        "value":{"type":"object","required":["task"],"properties":{"preparation":{"type":"object"},"task":{
            "type":"object","required":["task_id","plan_id","operation","project_context","targets","sources","start_revision","end_revision","status","phase","completed","total","failure_code","message","produced","modified","summary"],
            "properties":{"task_id":{"type":"string"},"plan_id":{"type":["string","null"]},"operation":{"type":"string"},"project_context":{"type":"string"},"targets":{"type":"array"},"sources":{"type":"array"},"start_revision":{"type":"integer"},"end_revision":{"type":["integer","null"]},"status":{"enum":["queued","running","committing","succeeded","failed","cancelled","stale"]},"phase":{"type":"string"},"completed":{"type":["integer","null"],"minimum":0},"total":{"type":["integer","null"],"minimum":0},"failure_code":{"type":["string","null"]},"message":{"type":["string","null"]},"produced":{"type":"array"},"modified":{"type":"array"},"summary":{}}
        }}}
    }})
}
