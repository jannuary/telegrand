use gettextrs::gettext;
use gtk::{glib, pango, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::enums::MessageContent;

use crate::session::chat::{BoxedMessageContent, Message};
use crate::session::content::MessageIndicators;
use crate::utils::parse_formatted_text;

const INDICATORS_PLACEHOLDER: char = '\u{FFFC}';
const INDICATORS_SPACING: i32 = 6;

mod imp {
    use super::*;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-label.ui")]
    pub struct MessageLabel {
        pub indicators_size: RefCell<Option<(i32, i32)>>,
        #[template_child]
        pub overlay: TemplateChild<gtk::Overlay>,
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub indicators: TemplateChild<MessageIndicators>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageLabel {
        const NAME: &'static str = "ContentMessageLabel";
        type Type = super::MessageLabel;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageLabel {
        fn dispose(&self, _obj: &Self::Type) {
            self.overlay.unparent();
        }
    }

    impl WidgetImpl for MessageLabel {
        fn measure(
            &self,
            widget: &Self::Type,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (_, indicators_size) = self.indicators.preferred_size();

            if let Some(old_indicators_size) = self.indicators_size.borrow().as_ref() {
                if indicators_size.width != old_indicators_size.0
                    || indicators_size.height != old_indicators_size.1
                {
                    widget.update_label_attributes(&indicators_size);
                }
            } else {
                widget.update_label_attributes(&indicators_size);
            }

            self.indicators_size
                .replace(Some((indicators_size.width, indicators_size.height)));

            let (mut minimum, mut natural, minimum_baseline, natural_baseline) =
                self.label.measure(orientation, for_size);

            let (indicators_min, indicators_nat, _, _) =
                self.indicators.measure(orientation, for_size);

            minimum = minimum.max(indicators_min);
            natural = natural.max(indicators_nat);

            if let gtk::Orientation::Vertical = orientation {
                let widget_direction = widget.direction();
                let text_direction = self.label.layout().direction(0);

                // If the widget direction is the opposite of the text direction, make
                // space for the indicators to the bottom
                if (matches!(widget_direction, gtk::TextDirection::Ltr)
                    && matches!(text_direction, pango::Direction::Rtl))
                    || (matches!(widget_direction, gtk::TextDirection::Rtl)
                        && matches!(text_direction, pango::Direction::Ltr))
                {
                    minimum += indicators_min;
                    natural += indicators_nat;
                }
            }

            (minimum, natural, minimum_baseline, natural_baseline)
        }

        fn size_allocate(&self, _widget: &Self::Type, width: i32, height: i32, baseline: i32) {
            self.overlay.allocate(width, height, baseline, None);
        }

        fn request_mode(&self, _widget: &Self::Type) -> gtk::SizeRequestMode {
            self.label.request_mode()
        }
    }
}

glib::wrapper! {
    pub struct MessageLabel(ObjectSubclass<imp::MessageLabel>)
        @extends gtk::Widget;
}

impl Default for MessageLabel {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageLabel {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageLabel")
    }

    fn update_label_attributes(&self, indicators_size: &gtk::Requisition) {
        let self_ = imp::MessageLabel::from_instance(self);

        if let Some(start_index) = self_.label.text().find(INDICATORS_PLACEHOLDER) {
            let attrs = pango::AttrList::new();
            let width = indicators_size.width + INDICATORS_SPACING;
            let height = indicators_size.height;
            let logical_rect = pango::Rectangle::new(
                0,
                -(height - (height / 4)) * pango::SCALE,
                width * pango::SCALE,
                height * pango::SCALE,
            );
            let mut shape = pango::Attribute::new_shape(&logical_rect, &logical_rect);

            shape.set_start_index(start_index as u32);
            shape.set_end_index((start_index + INDICATORS_PLACEHOLDER.len_utf8()) as u32);

            attrs.insert(shape);
            self_.label.set_attributes(Some(&attrs));
        } else {
            self_.label.set_attributes(None::<&pango::AttrList>);
        }
    }

    pub fn set_message(&self, message: &Message) {
        let self_ = imp::MessageLabel::from_instance(self);
        self_.indicators.set_message(message);

        let message_expression = gtk::ConstantExpression::new(message);
        let content_expression = gtk::PropertyExpression::new(
            Message::static_type(),
            Some(&message_expression),
            "content",
        );
        let text_expression = gtk::ClosureExpression::new(
            move |args| -> String {
                let content = args[1].get::<BoxedMessageContent>().unwrap();
                let text = format_message_content_text(content.0);
                let direction = pango::find_base_dir(&text);

                if let pango::Direction::Rtl = direction {
                    text
                } else {
                    format!("{}{}", text, INDICATORS_PLACEHOLDER)
                }
            },
            &[content_expression.upcast()],
        );
        text_expression.bind(&*self_.label, "label", gtk::NONE_WIDGET);
    }
}

fn format_message_content_text(content: MessageContent) -> String {
    match content {
        MessageContent::MessageText(content) => parse_formatted_text(content.text),
        _ => format!("<i>{}</i>", gettext("This message is unsupported")),
    }
}
