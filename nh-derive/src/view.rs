
use darling::{FromVariant, FromDeriveInput};
use proc_macro::{self, TokenStream};
use quote::quote;

#[derive(FromDeriveInput)]
#[darling(attributes(view))]
struct DeriveViewOpts {
    default_passthrough: String,
    domain: syn::Path,
}

#[derive(FromVariant)]
#[darling(attributes(view))]
struct DeriveViewVariantOpts {
    passthrough: Option<darling::util::Override<String>>,
}

pub fn derive_view(input: TokenStream) -> TokenStream {
    let input_ast = syn::parse_macro_input!(input);
    let opts = match DeriveViewOpts::from_derive_input(&input_ast) {
        Ok(opts) => opts,
        Err(e) => return e.write_errors().into(),
    };
    let syn::Data::Enum(data_enum) = &input_ast.data else {
        return syn::Error::new(
            input_ast.ident.span(),
            "View can currently only be derived for enums",
        )
        .to_compile_error()
        .into();
    };

    let arms = data_enum.variants.iter()
        .flat_map(|v| DeriveViewVariantOpts::from_variant(v).map(|e| (v, e))).collect::<Vec<_>>();
    let arms2 = arms.iter()
            .map(|e| (e.0, match &e.1.passthrough {
                Some(darling::util::Override::Explicit(p)) => &p,
                _ => &opts.default_passthrough,
            }))
            .map(|e| {
                let variant_ident = &e.0.ident;
                (quote! { Self::#variant_ident(inner) }, e.1)
            }).collect::<Vec<_>>();
    let arms_immutable = arms2.iter().map(|e| {
        let arm_matcher = &e.0;
        match e.1.as_str() {
            "eref" => quote! { #arm_matcher => inner.read() },
            "bare" | _ => quote! { #arm_matcher => inner },
        }
    }).collect::<Vec<_>>();
    let arms_mutable = arms2.iter().map(|e| {
        let arm_matcher = &e.0;
        match e.1.as_str() {
            "eref" => quote! { #arm_matcher => inner.write() },
            "bare" | _ => quote! { #arm_matcher => inner },
        }
    }).collect::<Vec<_>>();

    let arms_tagged_uuid = arms_immutable.iter().map(|e| quote! { #e.tagged_uuid() }).collect::<Vec<_>>();
    let arms_uuid = arms_immutable.iter().map(|e| quote! { #e.uuid() }).collect::<Vec<_>>();
    let arms_model_uuid = arms_immutable.iter().map(|e| quote! { #e.model_uuid() }).collect::<Vec<_>>();

    let arms_model = arms_immutable.iter().map(|e| quote! { #e.model() }).collect::<Vec<_>>();
    let arms_min_shape = arms_immutable.iter().map(|e| quote! { #e.min_shape() }).collect::<Vec<_>>();
    let arms_bounding_box = arms_immutable.iter().map(|e| quote! { #e.bounding_box() }).collect::<Vec<_>>();
    let arms_position = arms_immutable.iter().map(|e| quote! { #e.position() }).collect::<Vec<_>>();

    let arms_controller_for = arms_immutable.iter().map(|e| quote! { #e.controller_for(uuid) }).collect::<Vec<_>>();

    let arms_show_properties = arms_mutable.iter().map(|e| quote! { #e.show_properties(drawing_context, q, lp, ui, commands) }).collect::<Vec<_>>();
    let arms_draw_in = arms_mutable.iter().map(|e| quote! { #e.draw_in(q, context, canvas, tool) }).collect::<Vec<_>>();
    let arms_collect_alignment = arms_mutable.iter().map(|e| quote! { #e.collect_allignment(am) }).collect::<Vec<_>>();
    let arms_handle_event = arms_mutable.iter().map(|e| quote! { #e.handle_event(event, ehc, tool, element_setup_modal, commands) }).collect::<Vec<_>>();
    let arms_apply_command = arms_mutable.iter().map(|e| quote! { #e.apply_command(command, undo_accumulator, affected_models) }).collect::<Vec<_>>();
    let arms_refresh_buffers = arms_mutable.iter().map(|e| quote! { #e.refresh_buffers() }).collect::<Vec<_>>();
    let arms_head_count = arms_mutable.iter().map(|e| quote! { #e.head_count(flattened_views, flattened_views_status, flattened_represented_models) }).collect::<Vec<_>>();
    let arms_collect_model_uuids = arms_immutable.iter().map(|e| quote! { #e.collect_model_uuids(into) }).collect::<Vec<_>>();
    let arms_delete_when = arms_immutable.iter().map(|e| quote! { #e.delete_when(deleting) }).collect::<Vec<_>>();
    let arms_deep_copy_walk = arms_immutable.iter().map(|e| quote! { #e.deep_copy_walk(requested, uuid_present, tlc, c, m) }).collect::<Vec<_>>();
    let arms_deep_copy_clone = arms_immutable.iter().map(|e| quote! { #e.deep_copy_clone(uuid_present, tlc, c, m) }).collect::<Vec<_>>();
    let arms_deep_copy_relink = arms_mutable.iter().map(|e| quote! { #e.deep_copy_relink(c, m) }).collect::<Vec<_>>();

    let ident = input_ast.ident;
    let domain = opts.domain;

    let output = quote! {
        impl crate::common::entity::Entity for #ident {
            fn tagged_uuid(&self) -> crate::common::entity::EntityUuid {
                match self {
                    #(#arms_tagged_uuid),*
                }
            }
        }

        impl View for #ident {
            fn uuid(&self) -> Arc<ViewUuid> {
                match self {
                    #(#arms_uuid),*
                }
            }

            fn model_uuid(&self) -> Arc<ModelUuid> {
                match self {
                    #(#arms_model_uuid),*
                }
            }
        }

        impl ElementController<<#domain as crate::common::controller::Domain> :: CommonElementT> for #ident {
            fn model(&self) -> <#domain as crate::common::controller::Domain> :: CommonElementT {
                match self {
                    #(#arms_model),*
                }
            }
            fn min_shape(&self) -> NHShape {
                match self {
                    #(#arms_min_shape),*
                }
            }
            fn bounding_box(&self) -> egui::Rect {
                match self {
                    #(#arms_bounding_box),*
                }
            }
            fn position(&self) -> egui::Pos2 {
                match self {
                    #(#arms_position),*
                }
            }
        }

        impl ContainerGen2<#domain> for #ident {
            fn controller_for(&self, uuid: &ModelUuid) -> Option<<#domain as crate::common::controller::Domain> :: CommonElementViewT> {
                match self {
                    #(#arms_controller_for),*
                }
            }
        }

        impl ElementControllerGen2<#domain> for #ident {
            fn show_properties(
                &mut self,
                drawing_context: &GlobalDrawingContext,
                q: &<#domain as crate::common::controller::Domain> :: QueryableT<'_>,
                lp: &<#domain as crate::common::controller::Domain> :: LabelProviderT,
                ui: &mut egui::Ui,
                commands: &mut Vec<SensitiveCommand<<#domain as crate::common::controller::Domain> :: AddCommandElementT, <#domain as crate::common::controller::Domain> :: PropChangeT>>,
            ) -> PropertiesStatus<#domain> {
                match self {
                    #(#arms_show_properties),*
                }
            }
            fn draw_in(
                &mut self,
                q: &<#domain as crate::common::controller::Domain> :: QueryableT<'_>,
                context: &GlobalDrawingContext,
                canvas: &mut dyn canvas::NHCanvas,
                tool: &Option<(egui::Pos2, &<#domain as crate::common::controller::Domain> :: ToolT)>,
            ) -> TargettingStatus {
                match self {
                    #(#arms_draw_in),*
                }
            }
            fn collect_allignment(&mut self, am: &mut SnapManager) {
                match self {
                    #(#arms_collect_alignment),*
                }
            }
            fn handle_event(
                &mut self,
                event: InputEvent,
                ehc: &EventHandlingContext,
                tool: &mut Option<<#domain as crate::common::controller::Domain> :: ToolT>,
                element_setup_modal: &mut Option<Box<dyn CustomModal>>,
                commands: &mut Vec<SensitiveCommand<<#domain as crate::common::controller::Domain> :: AddCommandElementT, <#domain as crate::common::controller::Domain> :: PropChangeT>>,
            ) -> EventHandlingStatus {
                match self {
                    #(#arms_handle_event),*
                }
            }
            fn apply_command(
                &mut self,
                command: &InsensitiveCommand<<#domain as crate::common::controller::Domain> :: AddCommandElementT, <#domain as crate::common::controller::Domain> :: PropChangeT>,
                undo_accumulator: &mut Vec<InsensitiveCommand<<#domain as crate::common::controller::Domain> :: AddCommandElementT, <#domain as crate::common::controller::Domain> :: PropChangeT>>,
                affected_models: &mut HashSet<ModelUuid>,
            ) {
                match self {
                    #(#arms_apply_command),*
                }
            }
            fn refresh_buffers(&mut self) {
                match self {
                    #(#arms_refresh_buffers),*
                }
            }
            fn head_count(
                &mut self,
                flattened_views: &mut HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                flattened_views_status: &mut HashMap<ViewUuid, SelectionStatus>,
                flattened_represented_models: &mut HashMap<ModelUuid, ViewUuid>,
            ) {
                match self {
                    #(#arms_head_count),*
                }
            }
            fn collect_model_uuids(&self, into: &mut HashSet<ModelUuid>) {
                match self {
                    #(#arms_collect_model_uuids),*
                }
            }
            fn delete_when(&self, deleting: &HashSet<ViewUuid>) -> bool {
                match self {
                    #(#arms_delete_when),*
                }
            }
            fn deep_copy_walk(
                &self,
                requested: Option<&HashSet<ViewUuid>>,
                uuid_present: &dyn Fn(&ViewUuid) -> bool,
                tlc: &mut HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                c: &mut HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                m: &mut HashMap<ModelUuid, <#domain as crate::common::controller::Domain> :: CommonElementT>,
            ) {
                match self {
                    #(#arms_deep_copy_walk),*
                }
            }
            fn deep_copy_clone(
                &self,
                uuid_present: &dyn Fn(&ViewUuid) -> bool,
                tlc: &mut HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                c: &mut HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                m: &mut HashMap<ModelUuid, <#domain as crate::common::controller::Domain> :: CommonElementT>,
            ) {
                match self {
                    #(#arms_deep_copy_clone),*
                }
            }
            fn deep_copy_relink(
                &mut self,
                c: &HashMap<ViewUuid, <#domain as crate::common::controller::Domain> :: CommonElementViewT>,
                m: &HashMap<ModelUuid, <#domain as crate::common::controller::Domain> :: CommonElementT>,
            ) {
                match self {
                    #(#arms_deep_copy_relink),*
                }
            }
        }
    };
    output.into()
}
