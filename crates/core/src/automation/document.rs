//! Bounded, identity-addressed document discovery. No selection or data samples.
use super::*;
use crate::state::{CanvasDocument, ContentItem, ContentKind, Panel, PlotiumApp, SeriesBinding};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) mod source;

pub const DOCUMENT_INSPECT: &str = "document.inspect";
pub const KIND_PANEL: &str = "plotium.canvas.panel";
pub const DOCUMENT_PAGE_DEFAULT: usize = 50;
pub const DOCUMENT_PAGE_MAX: usize = 200;
pub const DOCUMENT_RESPONSE_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentView {
    #[serde(rename = "self")]
    SelfOnly,
    Children,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentInspectParams {
    pub target: TargetRef,
    pub view: DocumentView,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "page_default")]
    pub limit: usize,
}
fn page_default() -> usize {
    DOCUMENT_PAGE_DEFAULT
}

pub fn document_inspect_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["target","view"],
        "properties":{
            "target":{"type":"object","additionalProperties":false,"required":["resource"],"properties":{
                "resource":{"type":"object","additionalProperties":false,"required":["id","kind"],"properties":{
                    "id":{"type":"string"},"kind":{"type":"string"},
                    "parent_id":{"type":["string","null"]},"local_id":{"type":["string","null"]}}},
                "component":{"type":"object","additionalProperties":false,"required":["kind","id"],"properties":{
                    "kind":{"const":"series"},"id":{"type":"integer","minimum":0,"maximum":u64::MAX}}}}},
            "view":{"type":"string","enum":["self","children"]},
            "offset":{"type":"integer","minimum":0,"default":0},
            "limit":{"type":"integer","minimum":1,"maximum":DOCUMENT_PAGE_MAX,"default":DOCUMENT_PAGE_DEFAULT}},
        "allOf":[{"if":{"properties":{"view":{"const":"self"}}},"then":{"properties":{"offset":{"const":0},"limit":{"const":DOCUMENT_PAGE_DEFAULT}}}}]})
}

pub fn document_root_target() -> TargetRef {
    TargetRef::resource(ResourceRef {
        id: DOCUMENT_RESOURCE_ID.into(),
        kind: ResourceKindId::new(KIND_DOCUMENT),
        parent_id: None,
        local_id: None,
    })
}
pub fn panel_resource(canvas: &CanvasDocument, panel: &Panel) -> ResourceRef {
    ResourceRef {
        id: format!("{}/panels/{}", canvas.resource_id, panel.id),
        kind: ResourceKindId::new(KIND_PANEL),
        parent_id: Some(canvas.resource_id.to_string()),
        local_id: Some(panel.id.to_string()),
    }
}
fn canvas_target(canvas: &CanvasDocument) -> TargetRef {
    TargetRef::resource(canvas.resource_id.into())
}
fn object_target(canvas: &CanvasDocument, item: &ContentItem) -> TargetRef {
    TargetRef::resource(canvas_object_ref(canvas.resource_id, item.id))
}
fn series_target(canvas: &CanvasDocument, item: &ContentItem, series: &SeriesBinding) -> TargetRef {
    TargetRef {
        resource: canvas_object_ref(canvas.resource_id, item.id),
        component: Some(ComponentRef::Series(series.id)),
    }
}

enum Node<'a> {
    Document,
    Canvas(&'a CanvasDocument),
    Panel(&'a CanvasDocument, &'a Panel),
    Content(&'a CanvasDocument, &'a ContentItem),
    Series(&'a CanvasDocument, &'a ContentItem, Box<SeriesBinding>),
}
impl Node<'_> {
    fn target(&self) -> TargetRef {
        match self {
            Self::Document => document_root_target(),
            Self::Canvas(c) => canvas_target(c),
            Self::Panel(c, p) => TargetRef::resource(panel_resource(c, p)),
            Self::Content(c, o) => object_target(c, o),
            Self::Series(c, o, s) => series_target(c, o, s),
        }
    }
    fn children<'a>(&'a self, app: &'a PlotiumApp) -> Box<dyn Iterator<Item = Node<'a>> + 'a> {
        match self {
            Self::Document => Box::new(app.doc.canvases.iter().map(Node::Canvas)),
            Self::Canvas(c) => Box::new(
                c.panels.iter().map(|p| Node::Panel(c, p)).chain(
                    c.objects
                        .iter()
                        .filter(|o| c.parent_panel(o.id).is_none())
                        .map(|o| Node::Content(c, o)),
                ),
            ),
            Self::Panel(c, p) => Box::new(
                p.item_order
                    .iter()
                    .filter_map(|id| c.object(*id))
                    .map(|o| Node::Content(c, o)),
            ),
            Self::Content(c, o) => Box::new(
                o.plot()
                    .into_iter()
                    .flat_map(|p| app.display_binding(p.display_owner, &p.binding).series)
                    .map(|s| Node::Series(c, o, Box::new(s))),
            ),
            Self::Series(..) => Box::new(std::iter::empty()),
        }
    }
    fn summary(&self) -> Value {
        let (kind, name, visible) = match self {
            Self::Document => ("document", "Plotium document", true),
            Self::Canvas(c) => ("canvas", c.name.as_str(), true),
            Self::Panel(_, p) => ("panel", p.name.as_str(), p.visible),
            Self::Content(_, o) => (
                match o.kind {
                    ContentKind::Plot(_) => "plot",
                    ContentKind::Text(_) => "text",
                    ContentKind::Shape(_) => "shape",
                    ContentKind::RasterImage(_) => "raster_image",
                },
                o.name.as_str(),
                o.visible,
            ),
            Self::Series(_, _, s) => ("series", s.label.as_deref().unwrap_or(""), s.visible),
        };
        json!({"target":self.target(),"kind":kind,"name":name,"visible":visible})
    }
    fn detail(&self, app: &PlotiumApp) -> Value {
        let mut out = self.summary();
        let mut ancestors = vec![];
        if let Self::Canvas(c) | Self::Panel(c, _) | Self::Content(c, _) | Self::Series(c, _, _) =
            self
        {
            ancestors.push(document_root_target());
            if !matches!(self, Self::Canvas(_)) {
                ancestors.push(canvas_target(c));
            }
            if let Self::Content(_, o) | Self::Series(_, o, _) = self {
                if let Some(p) = c.parent_panel(o.id).and_then(|id| c.panel(id)) {
                    ancestors.push(TargetRef::resource(panel_resource(c, p)));
                }
                if matches!(self, Self::Series(..)) {
                    ancestors.push(object_target(c, o));
                }
            }
        }
        out["ancestors"] = json!(ancestors);
        out["child_count"] = json!(self.children(app).count());
        match self {
            Self::Canvas(c) => {
                out["size_mm"] = json!(c.size_mm);
            }
            Self::Panel(_, p) => {
                out["frame"] = frame(p.frame);
                out["locked"] = json!(p.locked);
            }
            Self::Content(c, o) => {
                out.as_object_mut().expect("local detail object").extend(
                    content_geometry_detail(c, o)
                        .as_object()
                        .expect("geometry object")
                        .clone(),
                );
                out["locked"] = json!(o.locked);
                match &o.kind {
                    ContentKind::Plot(p) => {
                        out["coordinate_units"] = json!(p.binding.coordinates);
                        out["chart_id"] = json!(p.chart.type_id);
                        out["chart_column"] = json!(p.chart.column);
                        out["annotations"] = json!(
                            p.axis_overrides
                                .annotations
                                .items
                                .iter()
                                .map(crate::state::annotation_reading)
                                .collect::<Vec<_>>()
                        );
                    }
                    ContentKind::Text(text) => {
                        out["text"] = json!(text.text);
                        out["font_size_pt"] = json!(text.font_size);
                    }
                    ContentKind::RasterImage(image) => {
                        out["asset_id"] = json!(image.asset);
                        out["image"] = json!({
                            "page_index":image.page_index,"crop":image.crop,
                            "fit":match image.fit { crate::state::ImageFit::Contain=>"contain",crate::state::ImageFit::Cover=>"cover",crate::state::ImageFit::Stretch=>"stretch" },
                            "rotation":match image.rotation { crate::state::QuarterTurn::Zero=>0,crate::state::QuarterTurn::Clockwise90=>90,crate::state::QuarterTurn::Clockwise180=>180,crate::state::QuarterTurn::Clockwise270=>270 },
                            "opacity":image.opacity,"interpolation":match image.interpolation {crate::state::ImageInterpolation::Auto=>"auto",crate::state::ImageInterpolation::Nearest=>"nearest",crate::state::ImageInterpolation::Linear=>"linear"},
                            "preserve_aspect":image.preserve_aspect,
                            "replaceable":c.validate_content_edit(o.id,false).is_ok() && image.page_index==0,
                            "replace_error":c.validate_content_edit(o.id,false).err().or_else(||(image.page_index!=0).then(||"PNG/JPEG replacement requires page_index 0; the existing page index is never reset.".to_owned()))
                        });
                    }
                    ContentKind::Shape(shape) => {
                        out["shape"] = json!(match shape.shape {
                            crate::state::ShapeKind::Rect => "rect",
                            crate::state::ShapeKind::Ellipse => "ellipse",
                            crate::state::ShapeKind::Line => "line",
                            crate::state::ShapeKind::Arrow => "arrow",
                        });
                    }
                }
            }
            Self::Series(_, o, s) => {
                out["coordinate_units"] =
                    json!(o.plot().and_then(|p| p.binding.coordinates.as_ref()));
                out["label"] = json!(s.label);
                out["encoding"] = json!(crate::properties::EncodingKind::of(&s.encoding).as_str());
                out["source"] = source::describe(app, s);
            }
            Self::Document => {}
        }
        out
    }
}
pub(crate) fn content_geometry_detail(c: &CanvasDocument, o: &ContentItem) -> Value {
    let parent = c.parent_panel(o.id).and_then(|id| c.panel(id));
    let preserve_aspect =
        matches!(&o.kind, ContentKind::RasterImage(image) if image.preserve_aspect);
    let error = c.validate_content_edit(o.id, true).err();
    json!({
        "parent":parent.map(|p|TargetRef::resource(panel_resource(c,p))).unwrap_or_else(||canvas_target(c)),
        "frame":frame(o.frame),
        "frame_space":if parent.is_some(){"panel_local_pt"}else{"page_pt"},
        "page_frame":c.content_page_frame(o.id).map(frame),
        "geometry":{"min_size_pt":[crate::state::MIN_CONTENT_FRAME_PT,crate::state::MIN_CONTENT_FRAME_PT],
            "preserve_aspect":preserve_aspect,
            "aspect_ratio":preserve_aspect.then_some(o.frame.width / o.frame.height),
            "frame_editable":error.is_none(),"frame_edit_error":error}
    })
}
fn frame(f: crate::state::ObjectFrame) -> Value {
    json!({"x":f.x,"y":f.y,"width":f.width,"height":f.height,"unit":"pt"})
}
fn invalid(message: impl Into<String>) -> AutomationError {
    AutomationError::InvalidSelector(message.into())
}
fn resolve<'a>(app: &'a PlotiumApp, target: &TargetRef) -> Result<Node<'a>, AutomationError> {
    if *target == document_root_target() {
        return Ok(Node::Document);
    }
    let resource = &target.resource;
    let c = app
        .doc
        .canvases
        .iter()
        .find(|c| {
            resource.id == c.resource_id.to_string()
                || resource.parent_id.as_deref() == Some(c.resource_id.to_string().as_str())
        })
        .ok_or_else(|| invalid("document target does not exist"))?;
    let node = if resource.kind.0 == KIND_CANVAS && target.component.is_none() {
        Node::Canvas(c)
    } else if resource.kind.0 == KIND_PANEL && target.component.is_none() {
        let p = c
            .panels
            .iter()
            .find(|p| resource.local_id.as_deref() == Some(p.id.to_string().as_str()))
            .ok_or_else(|| invalid("panel does not exist"))?;
        Node::Panel(c, p)
    } else if resource.kind.0 == KIND_CANVAS_OBJECT {
        let o = c
            .objects
            .iter()
            .find(|o| resource.local_id.as_deref() == Some(o.id.to_string().as_str()))
            .ok_or_else(|| invalid("content does not exist"))?;
        match target.component {
            None => Node::Content(c, o),
            Some(ComponentRef::Series(id)) => {
                let s = o
                    .plot()
                    .and_then(|p| {
                        app.display_binding(p.display_owner, &p.binding)
                            .series
                            .into_iter()
                            .find(|s| s.id == id)
                    })
                    .ok_or_else(|| {
                        invalid("series does not exist in this plot's current display binding")
                    })?;
                Node::Series(c, o, Box::new(s))
            }
            _ => return Err(invalid("only Series components are supported")),
        }
    } else {
        return Err(invalid("unsupported document target kind"));
    };
    if node.target() != *target {
        return Err(invalid("target identity does not match its owner"));
    }
    Ok(node)
}

pub(super) fn freeze(
    app: &PlotiumApp,
    request: &ToolRequest,
) -> Result<FrozenTargetSet, AutomationError> {
    let p: DocumentInspectParams =
        super::registry::parse(DOCUMENT_INSPECT, request.parameters.clone())?;
    if !(1..=DOCUMENT_PAGE_MAX).contains(&p.limit)
        || (p.view == DocumentView::SelfOnly && (p.offset != 0 || p.limit != DOCUMENT_PAGE_DEFAULT))
    {
        return Err(invalid(
            "limit must be 1..=200; pagination is only valid for children",
        ));
    }
    resolve(app, &p.target)?;
    let expected = if p.target.component.is_some() {
        TargetSelector::Components {
            targets: vec![p.target.clone()],
        }
    } else {
        TargetSelector::Explicit {
            ids: vec![p.target.resource.id.clone()],
        }
    };
    if request.targets != expected {
        return Err(invalid("selector must match the explicit document target"));
    }
    Ok(FrozenTargetSet {
        revision: DocumentRevision(app.doc.automation_revision),
        targets: vec![p.target.resource.clone()],
        component_targets: if p.target.component.is_some() {
            vec![p.target]
        } else {
            vec![]
        },
        reasons: vec![],
        total_matches: 1,
        truncated: false,
    })
}

pub(super) fn execute(app: &PlotiumApp, plan: &ToolPlan) -> Result<ToolResult, AutomationError> {
    let p: DocumentInspectParams =
        super::registry::parse(DOCUMENT_INSPECT, plan.request.parameters.clone())?;
    let node = resolve(app, &p.target)?;
    let revision = DocumentRevision(app.doc.automation_revision);
    let wrap = |v| super::tool_executors::readonly_value(DOCUMENT_INSPECT, revision, v);
    let size = |r: &ToolResult| {
        serde_json::to_vec(r)
            .map(|v| v.len())
            .map_err(|e| AutomationError::Execution(e.to_string()))
    };
    let too_large = || AutomationError::ResponseTooLarge;
    if p.view == DocumentView::SelfOnly {
        let result = wrap(json!({"revision":revision,"node":node.detail(app)}));
        if size(&result)? > DOCUMENT_RESPONSE_BYTES {
            return Err(too_large());
        }
        return Ok(result);
    }
    let total = node.children(app).count();
    let mut items = Vec::new();
    let page = |items: &Vec<Value>| {
        let next = p.offset.saturating_add(items.len());
        wrap(
            json!({"revision":revision,"target":p.target,"items":items,"total":total,
            "next_offset":if next<total{Some(next)}else{None},"truncated":next<total}),
        )
    };
    for child in node.children(app).skip(p.offset).take(p.limit) {
        items.push(child.summary());
        if size(&page(&items))? > DOCUMENT_RESPONSE_BYTES {
            items.pop();
            if items.is_empty() {
                return Err(too_large());
            }
            break;
        }
    }
    let result = page(&items);
    if size(&result)? > DOCUMENT_RESPONSE_BYTES {
        return Err(too_large());
    }
    Ok(result)
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
