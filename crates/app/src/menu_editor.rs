//! Tools → Customize Menu… (spec 37): the menus on the left, the chosen one's items on the right,
//! the chosen item's fields under. `core::menu::MenuEdit` holds the state; OK stores it.

use crate::app::Message;
use crate::fl;
use crate::menu::{entry, label, title};
use cosmic::iced::widget::{column, row};
use cosmic::iced::{Alignment, Length};
use cosmic::{Element, theme, widget};
use shagshell_core::menu::{Menu, MenuEdit, MenuMsg};

#[derive(Clone, Debug)]
pub enum MeMsg {
    Edit(MenuMsg),
    /// «Функция ▾»: the n-th of `Editor::functions`.
    Function(usize),
    /// «В меню ▾»: the n-th menu.
    MoveTo(usize),
    Ok,
    Cancel,
}

pub struct Editor {
    pub edit: MenuEdit,
    /// Every function (`menu::FUNCTIONS`) and its labels for the dropdown.
    functions: Vec<&'static str>,
    function_labels: Vec<String>,
    /// The menus' names, for «В меню ▾» (kept in step with `edit`).
    titles: Vec<String>,
}

impl Editor {
    pub fn new(saved: &[Menu]) -> Self {
        let functions = crate::menu::FUNCTIONS.clone();
        let function_labels = (functions.iter())
            .map(|c| crate::menu::function_choice(c))
            .collect();
        let mut e = Self {
            edit: MenuEdit::new(saved),
            functions,
            function_labels,
            titles: Vec::new(),
        };
        e.retitle();
        e
    }

    fn retitle(&mut self) {
        self.titles = self.edit.list.iter().map(title).collect();
    }

    /// An edit (OK / Cancel are the app's).
    pub fn update(&mut self, m: MeMsg) {
        let m = match m {
            MeMsg::Edit(m) => m,
            MeMsg::Function(i) => match self.functions.get(i) {
                Some(c) => MenuMsg::AddFunction((*c).into()),
                None => return,
            },
            MeMsg::MoveTo(i) => MenuMsg::MoveTo(i),
            MeMsg::Ok | MeMsg::Cancel => return,
        };
        self.edit.update(m);
        self.retitle();
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let msg = |m| Message::MenuEdit(MeMsg::Edit(m));
        let e = &self.edit;
        let pick = |selected: bool, content: Element<'static, Message>, on: Message| {
            widget::button::custom(
                widget::container(content)
                    .padding([2, 4])
                    .width(Length::Fill),
            )
            .class(match selected {
                true => theme::Button::Standard,
                false => theme::Button::MenuItem,
            })
            .width(Length::Fill)
            .on_press(on)
        };
        let check = |on: bool, can: bool, f: Box<dyn Fn(bool) -> Message>| {
            let mut c = widget::checkbox(on);
            if can || !on {
                c = c.on_toggle(f);
            }
            c
        };
        let card = |col: Element<'a, Message>| {
            widget::container(widget::scrollable(col).height(Length::Fixed(300.0)))
                .padding(4)
                .width(Length::Fill)
                .class(theme::Container::Card)
        };
        let arrow = |name: &'static str, m: MenuMsg, on: bool| {
            widget::button::icon(widget::icon::from_name(name).size(16))
                .on_press_maybe(on.then(|| msg(m)))
        };

        let mut menus = column![].spacing(1);
        for (i, m) in e.list.iter().enumerate() {
            menus = menus.push(
                row![
                    check(
                        m.shown,
                        e.can_hide_menu(i),
                        Box::new(move |on| msg(MenuMsg::ShowMenu(i, on)))
                    ),
                    pick(
                        i == e.menu,
                        widget::text::body(self.titles[i].clone()).into(),
                        msg(MenuMsg::SelectMenu(i))
                    ),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            );
        }
        let n = e.list.len();
        let cur = e.list.get(e.menu);
        let menu_side = column![
            widget::text::heading(fl!("me-menus")),
            card(menus.into()),
            row![
                arrow("go-up-symbolic", MenuMsg::MenuUp, e.menu > 0),
                arrow("go-down-symbolic", MenuMsg::MenuDown, e.menu + 1 < n),
                widget::button::standard(fl!("me-add-menu")).on_press(msg(MenuMsg::AddMenu)),
                widget::button::standard(fl!("me-delete"))
                    .on_press_maybe(e.can_delete_menu().then(|| msg(MenuMsg::DeleteMenu))),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            widget::text_input(
                cur.map_or_else(String::new, |m| title(&Menu {
                    title: String::new(),
                    ..m.clone()
                })),
                cur.map_or("", |m| m.title.as_str()),
            )
            .on_input(move |s| msg(MenuMsg::MenuTitle(s))),
        ]
        .spacing(6)
        .width(Length::FillPortion(2));

        let items = cur.map_or(&[][..], |m| &m.items[..]);
        let mut list = column![].spacing(1);
        for (i, it) in items.iter().enumerate() {
            let content: Element<'static, Message> = match it.is_sep() {
                true => widget::container(widget::divider::horizontal::default())
                    .padding([8, 4])
                    .width(Length::Fill)
                    .into(),
                false => widget::text::body(label(it)).into(),
            };
            let line = pick(e.item == Some(i), content, msg(MenuMsg::SelectItem(i)));
            list = list.push(
                row![
                    check(
                        it.shown,
                        e.can_hide_item(i),
                        Box::new(move |on| msg(MenuMsg::ShowItem(i, on)))
                    ),
                    line,
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            );
        }
        let sel = e.item.filter(|&i| i < items.len());
        let chosen = sel.map(|i| &items[i]);
        let item_side = column![
            widget::text::heading(fl!("me-items")),
            card(list.into()),
            row![
                arrow(
                    "go-up-symbolic",
                    MenuMsg::ItemUp,
                    sel.is_some_and(|i| i > 0)
                ),
                arrow(
                    "go-down-symbolic",
                    MenuMsg::ItemDown,
                    sel.is_some_and(|i| i + 1 < items.len())
                ),
                widget::dropdown(&self.titles, None, |i| Message::MenuEdit(MeMsg::MoveTo(i)))
                    .placeholder(fl!("me-move-to")),
                widget::button::standard(fl!("me-delete"))
                    .on_press_maybe(e.can_delete_item().then(|| msg(MenuMsg::DeleteItem))),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
            row![
                widget::dropdown(&self.function_labels, None, |i| {
                    Message::MenuEdit(MeMsg::Function(i))
                })
                .placeholder(fl!("me-add-function")),
                widget::button::standard(fl!("me-add-program")).on_press(msg(MenuMsg::AddProgram)),
                widget::button::standard(fl!("me-add-sep")).on_press(msg(MenuMsg::AddSep)),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        ]
        .spacing(6)
        .width(Length::FillPortion(3));

        let field = |name: String, input: Element<'static, Message>| {
            row![widget::text::body(name).width(Length::Fixed(110.0)), input]
                .spacing(6)
                .align_y(Alignment::Center)
        };
        let mut fields = column![].spacing(6);
        if let Some(it) = chosen.filter(|it| !it.is_sep()) {
            let stock = entry(&it.cmd).map(|e| e.1);
            fields = fields.push(field(
                fl!("me-label"),
                widget::text_input(
                    stock.clone().unwrap_or_else(|| it.cmd.clone()),
                    it.label.clone(),
                )
                .on_input(move |s| msg(MenuMsg::Label(s)))
                .into(),
            ));
            if it.is_own() {
                fields = fields.push(
                    row![
                        field(
                            fl!("toolbar-command"),
                            widget::text_input(fl!("toolbar-command-hint"), it.cmd.clone())
                                .on_input(move |s| msg(MenuMsg::Cmd(s)))
                                .into(),
                        ),
                        widget::button::standard(fl!("so-browse"))
                            .on_press(Message::ScriptFile(crate::script::Slot::Menu)),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                );
                fields = fields.push(field(
                    fl!("toolbar-params"),
                    widget::text_input(
                        match it.is_script() {
                            true => fl!("toolbar-params-script"),
                            false => fl!("toolbar-params-hint"),
                        },
                        it.params.clone(),
                    )
                    .on_input(move |s| msg(MenuMsg::Params(s)))
                    .into(),
                ));
            }
        }

        widget::dialog()
            .title(fl!("me-title"))
            .width(Length::Fixed(820.0))
            .max_width(820.0)
            .control(
                column![row![menu_side, item_side].spacing(16), fields]
                    .spacing(12)
                    .width(Length::Fill),
            )
            .primary_action(
                widget::button::suggested(fl!("ok")).on_press(Message::MenuEdit(MeMsg::Ok)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel")).on_press(Message::MenuEdit(MeMsg::Cancel)),
            )
            .tertiary_action(
                widget::button::standard(fl!("toolbar-reset")).on_press(msg(MenuMsg::Reset)),
            )
            .into()
    }
}
