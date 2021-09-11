use adw::{prelude::BinExt, subclass::prelude::BinImpl};
use gettextrs::gettext;
use gtk::{glib, pango, prelude::*, subclass::prelude::*, CompositeTemplate};
use tdgrand::enums::{ChatType, MessageContent, TextEntityType};
use tdgrand::types::FormattedText;

use crate::session::chat::{BoxedMessageContent, Message, MessageSender};
use crate::session::components::Avatar;
use crate::session::{Chat, User};
use crate::utils::{escape, linkify};

fn convert_to_markup(text: String, entity: &TextEntityType) -> String {
    match entity {
        TextEntityType::Url => format!("<a href='{}'>{}</a>", linkify(&text), text),
        TextEntityType::EmailAddress => format!("<a href='mailto:{0}'>{0}</a>", text),
        TextEntityType::PhoneNumber => format!("<a href='tel:{0}'>{0}</a>", text),
        TextEntityType::Bold => format!("<b>{}</b>", text),
        TextEntityType::Italic => format!("<i>{}</i>", text),
        TextEntityType::Underline => format!("<u>{}</u>", text),
        TextEntityType::Strikethrough => format!("<s>{}</s>", text),
        TextEntityType::Code | TextEntityType::Pre | TextEntityType::PreCode(_) => {
            format!("<tt>{}</tt>", text)
        }
        TextEntityType::TextUrl(data) => format!("<a href='{}'>{}</a>", escape(&data.url), text),
        _ => text,
    }
}

fn parse_formatted_text(formatted_text: FormattedText) -> String {
    let mut entities = formatted_text.entities.iter();
    let mut entity = entities.next();
    let mut output = String::new();
    let mut buffer = String::new();
    let mut is_inside_entity = false;

    // This is the offset in utf16 code units of the text to parse. We need this variable
    // because tdlib stores the offset and length parameters as utf16 code units instead
    // of regular code points.
    let mut code_units_offset = 0;

    for c in formatted_text.text.chars() {
        if !is_inside_entity
            && entity.is_some()
            && code_units_offset >= entity.unwrap().offset as usize
        {
            is_inside_entity = true;

            if !buffer.is_empty() {
                output.push_str(&escape(&buffer));
                buffer = String::new();
            }
        }

        buffer.push(c);
        code_units_offset += c.len_utf16();

        if let Some(entity_) = entity {
            if code_units_offset >= (entity_.offset + entity_.length) as usize {
                buffer = escape(&buffer);

                entity = loop {
                    let entity = entities.next();

                    // Handle eventual nested entities
                    match entity {
                        Some(entity) => {
                            if entity.offset == entity_.offset {
                                buffer = convert_to_markup(buffer, &entity.r#type);
                            } else {
                                break Some(entity);
                            }
                        }
                        None => break None,
                    }
                };

                output.push_str(&convert_to_markup(buffer, &entity_.r#type));
                buffer = String::new();
                is_inside_entity = false;
            }
        }
    }

    // Add the eventual leftovers from the buffer to the output
    if !buffer.is_empty() {
        output.push_str(&escape(&buffer));
    }

    output
}

fn format_message_content_text(content: MessageContent) -> String {
    match content {
        MessageContent::MessageText(content) => parse_formatted_text(content.text),
        _ => format!("<i>{}</i>", gettext("This message is unsupported")),
    }
}

mod imp {
    use super::*;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/github/melix99/telegrand/ui/content-message-row.ui")]
    pub struct MessageRow {
        #[template_child]
        pub avatar_bin: TemplateChild<adw::Bin>,
        #[template_child]
        pub content_bin: TemplateChild<adw::Bin>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for MessageRow {
        const NAME: &'static str = "ContentMessageRow";
        type Type = super::MessageRow;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for MessageRow {}
    impl WidgetImpl for MessageRow {}
    impl BinImpl for MessageRow {}
}

glib::wrapper! {
    pub struct MessageRow(ObjectSubclass<imp::MessageRow>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for MessageRow {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageRow {
    pub fn new() -> Self {
        glib::Object::new(&[]).expect("Failed to create MessageRow")
    }

    pub fn set_message(&self, message: &Message) {
        self.show_message_bubble(message);
    }

    fn show_message_bubble(&self, message: &Message) {
        let self_ = imp::MessageRow::from_instance(self);

        let vbox = gtk::BoxBuilder::new()
            .css_classes(vec!["message-bubble".to_string()])
            .orientation(gtk::Orientation::Vertical)
            .build();
        self_.content_bin.set_child(Some(&vbox));

        if message.is_outgoing() {
            self.set_halign(gtk::Align::End);
            vbox.add_css_class("outgoing");
        } else {
            self.set_halign(gtk::Align::Start);
            vbox.add_css_class("incoming");
        }

        self_.avatar_bin.set_child(None::<&gtk::Widget>);

        if !message.is_outgoing() {
            let is_channel = if let ChatType::Supergroup(data) = message.chat().type_() {
                data.is_channel
            } else {
                false
            };

            match message.chat().type_() {
                ChatType::BasicGroup(_) | ChatType::Supergroup(_) => {
                    let sender_label = MessageRow::create_sender_label(message);
                    vbox.append(&sender_label);

                    if !is_channel {
                        let sender_avatar = MessageRow::create_sender_avatar(message);
                        self_.avatar_bin.set_child(Some(&sender_avatar));

                        sender_label
                            .bind_property("label", &sender_avatar, "display-name")
                            .flags(glib::BindingFlags::SYNC_CREATE)
                            .build();
                    }
                }
                _ => {}
            }
        }

        let text_label = MessageRow::create_text_label(message);
        vbox.append(&text_label);
    }

    fn create_sender_label(message: &Message) -> gtk::Label {
        let label = gtk::LabelBuilder::new()
            .css_classes(vec!["sender-text".to_string()])
            .ellipsize(pango::EllipsizeMode::End)
            .single_line_mode(true)
            .xalign(0.0)
            .build();

        match message.sender() {
            MessageSender::User(user) => {
                let user_expression = gtk::ConstantExpression::new(&user);
                let first_name_expression = gtk::PropertyExpression::new(
                    User::static_type(),
                    Some(&user_expression),
                    "first-name",
                );
                let last_name_expression = gtk::PropertyExpression::new(
                    User::static_type(),
                    Some(&user_expression),
                    "last-name",
                );
                let full_name_expression = gtk::ClosureExpression::new(
                    move |expressions| -> String {
                        let first_name = expressions[1].get::<&str>().unwrap();
                        let last_name = expressions[2].get::<&str>().unwrap();
                        format!("{} {}", first_name, last_name).trim().to_string()
                    },
                    &[
                        first_name_expression.upcast(),
                        last_name_expression.upcast(),
                    ],
                );

                full_name_expression.bind(&label, "label", Some(&label));

                let classes = vec![
                    "sender-text-red".to_string(),
                    "sender-text-orange".to_string(),
                    "sender-text-violet".to_string(),
                    "sender-text-green".to_string(),
                    "sender-text-cyan".to_string(),
                    "sender-text-blue".to_string(),
                    "sender-text-pink".to_string(),
                ];

                let user_class = &classes[user.id() as usize % classes.len()];
                label.add_css_class(&user_class.to_string());
            }
            MessageSender::Chat(chat) => {
                let chat_expression = gtk::ConstantExpression::new(&chat);
                let title_expression = gtk::PropertyExpression::new(
                    Chat::static_type(),
                    Some(&chat_expression),
                    "title",
                );

                title_expression.bind(&label, "label", Some(&label));
            }
        }

        label
    }

    fn create_sender_avatar(message: &Message) -> Avatar {
        let sender_avatar = Avatar::new();
        sender_avatar.set_size(32);
        sender_avatar.set_valign(gtk::Align::End);

        match message.sender() {
            MessageSender::User(user) => sender_avatar.set_item(Some(user.avatar().clone())),
            MessageSender::Chat(chat) => sender_avatar.set_item(Some(chat.avatar().clone())),
        }

        sender_avatar
    }

    fn create_text_label(message: &Message) -> gtk::Label {
        let label = gtk::LabelBuilder::new()
            .css_classes(vec!["message-text".to_string()])
            .selectable(true)
            .use_markup(true)
            .wrap(true)
            .wrap_mode(pango::WrapMode::WordChar)
            .xalign(0.0)
            .build();

        let message_expression = gtk::ConstantExpression::new(message);
        let content_expression = gtk::PropertyExpression::new(
            Message::static_type(),
            Some(&message_expression),
            "content",
        );
        let text_expression = gtk::ClosureExpression::new(
            move |expressions| -> String {
                let content = expressions[1].get::<BoxedMessageContent>().unwrap();
                format_message_content_text(content.0)
            },
            &[content_expression.upcast()],
        );
        text_expression.bind(&label, "label", Some(&label));

        label
    }
}
