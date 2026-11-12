//! Telegram webhook payload fields. See the linked official definitions.
//! Independently modeled from the public contract; no upstream implementation is vendored.
use super::*;

model!(#[doc = "https://core.telegram.org/bots/api#animation"] TelegramAnimation {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    width: i64 = "width",
    height: i64 = "height",
    duration: i64 = "duration",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
    file_name: String = "file_name",
    mime_type: String = "mime_type",
    file_size: i64 = "file_size",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#areaposition"]
    TelegramAreaPosition {
        x_percentage: f64 = "x_percentage",
        y_percentage: f64 = "y_percentage",
        width_percentage: f64 = "width_percentage",
        height_percentage: f64 = "height_percentage",
        rotation_angle: f64 = "rotation_angle",
        corner_radius_percentage: f64 = "corner_radius_percentage",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#audio"] TelegramAudio {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    duration: i64 = "duration",
    performer: String = "performer",
    title: String = "title",
    file_name: String = "file_name",
    mime_type: String = "mime_type",
    file_size: i64 = "file_size",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
});

union!(#[doc = "https://core.telegram.org/bots/api#backgroundfill"] TelegramBackgroundFill, "type", {
    Solid(TelegramBackgroundFillSolid) = "solid",
    Gradient(TelegramBackgroundFillGradient) = "gradient",
    FreeformGradient(TelegramBackgroundFillFreeformGradient) = "freeform_gradient",
});

model!(#[doc = "https://core.telegram.org/bots/api#backgroundfillfreeformgradient"] TelegramBackgroundFillFreeformGradient {
    r#type: String = "type",
    colors: Vec<i64> = "colors",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#backgroundfillgradient"]
    TelegramBackgroundFillGradient {
        r#type: String = "type",
        top_color: i64 = "top_color",
        bottom_color: i64 = "bottom_color",
        rotation_angle: i64 = "rotation_angle",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#backgroundfillsolid"]
    TelegramBackgroundFillSolid {
        r#type: String = "type",
        color: i64 = "color",
    }
);

union!(#[doc = "https://core.telegram.org/bots/api#backgroundtype"] TelegramBackgroundType, "type", {
    Fill(TelegramBackgroundTypeFill) = "fill",
    Wallpaper(TelegramBackgroundTypeWallpaper) = "wallpaper",
    Pattern(TelegramBackgroundTypePattern) = "pattern",
    ChatTheme(TelegramBackgroundTypeChatTheme) = "chat_theme",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#backgroundtypechattheme"]
    TelegramBackgroundTypeChatTheme {
        r#type: String = "type",
        theme_name: String = "theme_name",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#backgroundtypefill"] TelegramBackgroundTypeFill {
    r#type: String = "type",
    fill: Box<TelegramBackgroundFill> = "fill",
    dark_theme_dimming: i64 = "dark_theme_dimming",
});

model!(#[doc = "https://core.telegram.org/bots/api#backgroundtypepattern"] TelegramBackgroundTypePattern {
    r#type: String = "type",
    document: Box<TelegramDocument> = "document",
    fill: Box<TelegramBackgroundFill> = "fill",
    intensity: i64 = "intensity",
    is_inverted: bool = "is_inverted",
    is_moving: bool = "is_moving",
});

model!(#[doc = "https://core.telegram.org/bots/api#backgroundtypewallpaper"] TelegramBackgroundTypeWallpaper {
    r#type: String = "type",
    document: Box<TelegramDocument> = "document",
    dark_theme_dimming: i64 = "dark_theme_dimming",
    is_blurred: bool = "is_blurred",
    is_moving: bool = "is_moving",
});

model!(#[doc = "https://core.telegram.org/bots/api#botsubscriptionupdated"] TelegramBotSubscriptionUpdated {
    user: Box<TelegramUser> = "user",
    invoice_payload: String = "invoice_payload",
    state: String = "state",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#businessbotrights"]
    TelegramBusinessBotRights {
        can_reply: bool = "can_reply",
        can_read_messages: bool = "can_read_messages",
        can_delete_sent_messages: bool = "can_delete_sent_messages",
        can_delete_all_messages: bool = "can_delete_all_messages",
        can_edit_name: bool = "can_edit_name",
        can_edit_bio: bool = "can_edit_bio",
        can_edit_profile_photo: bool = "can_edit_profile_photo",
        can_edit_username: bool = "can_edit_username",
        can_change_gift_settings: bool = "can_change_gift_settings",
        can_view_gifts_and_stars: bool = "can_view_gifts_and_stars",
        can_convert_gifts_to_stars: bool = "can_convert_gifts_to_stars",
        can_transfer_and_upgrade_gifts: bool = "can_transfer_and_upgrade_gifts",
        can_transfer_stars: bool = "can_transfer_stars",
        can_manage_stories: bool = "can_manage_stories",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#businessconnection"] TelegramBusinessConnection {
    id: String = "id",
    user: Box<TelegramUser> = "user",
    user_chat_id: i64 = "user_chat_id",
    date: i64 = "date",
    rights: Box<TelegramBusinessBotRights> = "rights",
    is_enabled: bool = "is_enabled",
});

model!(#[doc = "https://core.telegram.org/bots/api#businessmessagesdeleted"] TelegramBusinessMessagesDeleted {
    business_connection_id: String = "business_connection_id",
    chat: Box<TelegramChat> = "chat",
    message_ids: Vec<i64> = "message_ids",
});

model!(#[doc = "https://core.telegram.org/bots/api#callbackquery"] TelegramCallbackQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    message: Box<TelegramMaybeInaccessibleMessage> = "message",
    inline_message_id: String = "inline_message_id",
    chat_instance: String = "chat_instance",
    data: String = "data",
    game_short_name: String = "game_short_name",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#chat"]
    TelegramChat {
        id: i64 = "id",
        r#type: String = "type",
        title: String = "title",
        username: String = "username",
        first_name: String = "first_name",
        last_name: String = "last_name",
        is_forum: bool = "is_forum",
        is_direct_messages: bool = "is_direct_messages",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#chatbackground"] TelegramChatBackground {
    r#type: Box<TelegramBackgroundType> = "type",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatboost"] TelegramChatBoost {
    boost_id: String = "boost_id",
    add_date: i64 = "add_date",
    expiration_date: i64 = "expiration_date",
    source: Box<TelegramChatBoostSource> = "source",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#chatboostadded"]
    TelegramChatBoostAdded {
        boost_count: i64 = "boost_count",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#chatboostremoved"] TelegramChatBoostRemoved {
    chat: Box<TelegramChat> = "chat",
    boost_id: String = "boost_id",
    remove_date: i64 = "remove_date",
    source: Box<TelegramChatBoostSource> = "source",
});

union!(#[doc = "https://core.telegram.org/bots/api#chatboostsource"] TelegramChatBoostSource, "source", {
    Premium(TelegramChatBoostSourcePremium) = "premium",
    GiftCode(TelegramChatBoostSourceGiftCode) = "gift_code",
    Giveaway(TelegramChatBoostSourceGiveaway) = "giveaway",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatboostsourcegiftcode"] TelegramChatBoostSourceGiftCode {
    source: String = "source",
    user: Box<TelegramUser> = "user",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatboostsourcegiveaway"] TelegramChatBoostSourceGiveaway {
    source: String = "source",
    giveaway_message_id: i64 = "giveaway_message_id",
    user: Box<TelegramUser> = "user",
    prize_star_count: i64 = "prize_star_count",
    is_unclaimed: bool = "is_unclaimed",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatboostsourcepremium"] TelegramChatBoostSourcePremium {
    source: String = "source",
    user: Box<TelegramUser> = "user",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatboostupdated"] TelegramChatBoostUpdated {
    chat: Box<TelegramChat> = "chat",
    boost: Box<TelegramChatBoost> = "boost",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatinvitelink"] TelegramChatInviteLink {
    invite_link: String = "invite_link",
    creator: Box<TelegramUser> = "creator",
    creates_join_request: bool = "creates_join_request",
    is_primary: bool = "is_primary",
    is_revoked: bool = "is_revoked",
    name: String = "name",
    expire_date: i64 = "expire_date",
    member_limit: i64 = "member_limit",
    pending_join_request_count: i64 = "pending_join_request_count",
    subscription_period: i64 = "subscription_period",
    subscription_price: i64 = "subscription_price",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatjoinrequest"] TelegramChatJoinRequest {
    chat: Box<TelegramChat> = "chat",
    from: Box<TelegramUser> = "from",
    user_chat_id: i64 = "user_chat_id",
    date: i64 = "date",
    bio: String = "bio",
    invite_link: Box<TelegramChatInviteLink> = "invite_link",
    query_id: String = "query_id",
});

union!(#[doc = "https://core.telegram.org/bots/api#chatmember"] TelegramChatMember, "status", {
    Owner(TelegramChatMemberOwner) = "creator",
    Administrator(TelegramChatMemberAdministrator) = "administrator",
    Member(TelegramChatMemberMember) = "member",
    Restricted(TelegramChatMemberRestricted) = "restricted",
    Left(TelegramChatMemberLeft) = "left",
    Banned(TelegramChatMemberBanned) = "kicked",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberadministrator"] TelegramChatMemberAdministrator {
    status: String = "status",
    user: Box<TelegramUser> = "user",
    can_be_edited: bool = "can_be_edited",
    is_anonymous: bool = "is_anonymous",
    can_manage_chat: bool = "can_manage_chat",
    can_delete_messages: bool = "can_delete_messages",
    can_manage_video_chats: bool = "can_manage_video_chats",
    can_restrict_members: bool = "can_restrict_members",
    can_promote_members: bool = "can_promote_members",
    can_change_info: bool = "can_change_info",
    can_invite_users: bool = "can_invite_users",
    can_post_stories: bool = "can_post_stories",
    can_edit_stories: bool = "can_edit_stories",
    can_delete_stories: bool = "can_delete_stories",
    can_post_messages: bool = "can_post_messages",
    can_edit_messages: bool = "can_edit_messages",
    can_pin_messages: bool = "can_pin_messages",
    can_manage_topics: bool = "can_manage_topics",
    can_manage_direct_messages: bool = "can_manage_direct_messages",
    can_manage_tags: bool = "can_manage_tags",
    can_send_welcome_messages: bool = "can_send_welcome_messages",
    custom_title: String = "custom_title",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberbanned"] TelegramChatMemberBanned {
    status: String = "status",
    user: Box<TelegramUser> = "user",
    until_date: i64 = "until_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberleft"] TelegramChatMemberLeft {
    status: String = "status",
    user: Box<TelegramUser> = "user",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmembermember"] TelegramChatMemberMember {
    status: String = "status",
    tag: String = "tag",
    user: Box<TelegramUser> = "user",
    until_date: i64 = "until_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberowner"] TelegramChatMemberOwner {
    status: String = "status",
    user: Box<TelegramUser> = "user",
    is_anonymous: bool = "is_anonymous",
    custom_title: String = "custom_title",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberrestricted"] TelegramChatMemberRestricted {
    status: String = "status",
    tag: String = "tag",
    user: Box<TelegramUser> = "user",
    is_member: bool = "is_member",
    can_send_messages: bool = "can_send_messages",
    can_send_audios: bool = "can_send_audios",
    can_send_documents: bool = "can_send_documents",
    can_send_photos: bool = "can_send_photos",
    can_send_videos: bool = "can_send_videos",
    can_send_video_notes: bool = "can_send_video_notes",
    can_send_voice_notes: bool = "can_send_voice_notes",
    can_send_polls: bool = "can_send_polls",
    can_send_other_messages: bool = "can_send_other_messages",
    can_add_web_page_previews: bool = "can_add_web_page_previews",
    can_react_to_messages: bool = "can_react_to_messages",
    can_edit_tag: bool = "can_edit_tag",
    can_change_info: bool = "can_change_info",
    can_invite_users: bool = "can_invite_users",
    can_pin_messages: bool = "can_pin_messages",
    can_manage_topics: bool = "can_manage_topics",
    until_date: i64 = "until_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatmemberupdated"] TelegramChatMemberUpdated {
    chat: Box<TelegramChat> = "chat",
    from: Box<TelegramUser> = "from",
    date: i64 = "date",
    old_chat_member: Box<TelegramChatMember> = "old_chat_member",
    new_chat_member: Box<TelegramChatMember> = "new_chat_member",
    invite_link: Box<TelegramChatInviteLink> = "invite_link",
    via_join_request: bool = "via_join_request",
    via_chat_folder_invite_link: bool = "via_chat_folder_invite_link",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatownerchanged"] TelegramChatOwnerChanged {
    new_owner: Box<TelegramUser> = "new_owner",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatownerleft"] TelegramChatOwnerLeft {
    new_owner: Box<TelegramUser> = "new_owner",
});

model!(#[doc = "https://core.telegram.org/bots/api#chatshared"] TelegramChatShared {
    request_id: i64 = "request_id",
    chat_id: i64 = "chat_id",
    title: String = "title",
    username: String = "username",
    photo: Vec<TelegramPhotoSize> = "photo",
});

model!(#[doc = "https://core.telegram.org/bots/api#checklist"] TelegramChecklist {
    title: String = "title",
    title_entities: Vec<TelegramMessageEntity> = "title_entities",
    tasks: Vec<TelegramChecklistTask> = "tasks",
    others_can_add_tasks: bool = "others_can_add_tasks",
    others_can_mark_tasks_as_done: bool = "others_can_mark_tasks_as_done",
});

model!(#[doc = "https://core.telegram.org/bots/api#checklisttask"] TelegramChecklistTask {
    id: i64 = "id",
    text: String = "text",
    text_entities: Vec<TelegramMessageEntity> = "text_entities",
    completed_by_user: Box<TelegramUser> = "completed_by_user",
    completed_by_chat: Box<TelegramChat> = "completed_by_chat",
    completion_date: i64 = "completion_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#checklisttasksadded"] TelegramChecklistTasksAdded {
    checklist_message: Box<TelegramMessage> = "checklist_message",
    tasks: Vec<TelegramChecklistTask> = "tasks",
});

model!(#[doc = "https://core.telegram.org/bots/api#checklisttasksdone"] TelegramChecklistTasksDone {
    checklist_message: Box<TelegramMessage> = "checklist_message",
    marked_as_done_task_ids: Vec<i64> = "marked_as_done_task_ids",
    marked_as_not_done_task_ids: Vec<i64> = "marked_as_not_done_task_ids",
});

model!(#[doc = "https://core.telegram.org/bots/api#choseninlineresult"] TelegramChosenInlineResult {
    result_id: String = "result_id",
    from: Box<TelegramUser> = "from",
    location: Box<TelegramLocation> = "location",
    inline_message_id: String = "inline_message_id",
    query: String = "query",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#community"]
    TelegramCommunity {
        id: i64 = "id",
        name: String = "name",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#communitychatadded"] TelegramCommunityChatAdded {
    community: Box<TelegramCommunity> = "community",
});

model!(#[doc = "https://core.telegram.org/bots/api#communitychatjoined"] TelegramCommunityChatJoined {
    community: Box<TelegramCommunity> = "community",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#communitychatremoved"]
    TelegramCommunityChatRemoved {}
);

model!(
    #[doc = "https://core.telegram.org/bots/api#contact"]
    TelegramContact {
        phone_number: String = "phone_number",
        first_name: String = "first_name",
        last_name: String = "last_name",
        user_id: i64 = "user_id",
        vcard: String = "vcard",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#copytextbutton"]
    TelegramCopyTextButton {
        text: String = "text",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#dice"]
    TelegramDice {
        emoji: String = "emoji",
        value: i64 = "value",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#directmessagepricechanged"]
    TelegramDirectMessagePriceChanged {
        are_direct_messages_enabled: bool = "are_direct_messages_enabled",
        direct_message_star_count: i64 = "direct_message_star_count",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#directmessagestopic"] TelegramDirectMessagesTopic {
    topic_id: i64 = "topic_id",
    user: Box<TelegramUser> = "user",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#disabledbutton"]
    TelegramDisabledButton {}
);

model!(#[doc = "https://core.telegram.org/bots/api#document"] TelegramDocument {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
    file_name: String = "file_name",
    mime_type: String = "mime_type",
    file_size: i64 = "file_size",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#encryptedcredentials"]
    TelegramEncryptedCredentials {
        data: String = "data",
        hash: String = "hash",
        secret: String = "secret",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#encryptedpassportelement"] TelegramEncryptedPassportElement {
    r#type: String = "type",
    data: String = "data",
    phone_number: String = "phone_number",
    email: String = "email",
    files: Vec<TelegramPassportFile> = "files",
    front_side: Box<TelegramPassportFile> = "front_side",
    reverse_side: Box<TelegramPassportFile> = "reverse_side",
    selfie: Box<TelegramPassportFile> = "selfie",
    translation: Vec<TelegramPassportFile> = "translation",
    hash: String = "hash",
});

model!(#[doc = "https://core.telegram.org/bots/api#externalreplyinfo"] TelegramExternalReplyInfo {
    origin: Box<TelegramMessageOrigin> = "origin",
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    link_preview_options: Box<TelegramLinkPreviewOptions> = "link_preview_options",
    animation: Box<TelegramAnimation> = "animation",
    audio: Box<TelegramAudio> = "audio",
    document: Box<TelegramDocument> = "document",
    live_photo: Box<TelegramLivePhoto> = "live_photo",
    paid_media: Box<TelegramPaidMediaInfo> = "paid_media",
    photo: Vec<TelegramPhotoSize> = "photo",
    sticker: Box<TelegramSticker> = "sticker",
    story: Box<TelegramStory> = "story",
    video: Box<TelegramVideo> = "video",
    video_note: Box<TelegramVideoNote> = "video_note",
    voice: Box<TelegramVoice> = "voice",
    widget: Box<TelegramWidget> = "widget",
    has_media_spoiler: bool = "has_media_spoiler",
    checklist: Box<TelegramChecklist> = "checklist",
    contact: Box<TelegramContact> = "contact",
    dice: Box<TelegramDice> = "dice",
    game: Box<TelegramGame> = "game",
    giveaway: Box<TelegramGiveaway> = "giveaway",
    giveaway_winners: Box<TelegramGiveawayWinners> = "giveaway_winners",
    invoice: Box<TelegramInvoice> = "invoice",
    location: Box<TelegramLocation> = "location",
    poll: Box<TelegramPoll> = "poll",
    venue: Box<TelegramVenue> = "venue",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#file"]
    TelegramFile {
        file_id: String = "file_id",
        file_unique_id: String = "file_unique_id",
        file_size: i64 = "file_size",
        file_path: String = "file_path",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#forumtopicclosed"]
    TelegramForumTopicClosed {}
);

model!(
    #[doc = "https://core.telegram.org/bots/api#forumtopiccreated"]
    TelegramForumTopicCreated {
        name: String = "name",
        icon_color: i64 = "icon_color",
        icon_custom_emoji_id: String = "icon_custom_emoji_id",
        is_name_implicit: bool = "is_name_implicit",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#forumtopicedited"]
    TelegramForumTopicEdited {
        name: String = "name",
        icon_custom_emoji_id: String = "icon_custom_emoji_id",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#forumtopicreopened"]
    TelegramForumTopicReopened {}
);

model!(#[doc = "https://core.telegram.org/bots/api#game"] TelegramGame {
    title: String = "title",
    description: String = "description",
    photo: Vec<TelegramPhotoSize> = "photo",
    text: String = "text",
    text_entities: Vec<TelegramMessageEntity> = "text_entities",
    animation: Box<TelegramAnimation> = "animation",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#generalforumtopichidden"]
    TelegramGeneralForumTopicHidden {}
);

model!(
    #[doc = "https://core.telegram.org/bots/api#generalforumtopicunhidden"]
    TelegramGeneralForumTopicUnhidden {}
);

model!(#[doc = "https://core.telegram.org/bots/api#gift"] TelegramGift {
    id: String = "id",
    sticker: Box<TelegramSticker> = "sticker",
    star_count: i64 = "star_count",
    upgrade_star_count: i64 = "upgrade_star_count",
    is_premium: bool = "is_premium",
    has_colors: bool = "has_colors",
    total_count: i64 = "total_count",
    remaining_count: i64 = "remaining_count",
    personal_total_count: i64 = "personal_total_count",
    personal_remaining_count: i64 = "personal_remaining_count",
    background: Box<TelegramGiftBackground> = "background",
    unique_gift_variant_count: i64 = "unique_gift_variant_count",
    publisher_chat: Box<TelegramChat> = "publisher_chat",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#giftbackground"]
    TelegramGiftBackground {
        center_color: i64 = "center_color",
        edge_color: i64 = "edge_color",
        text_color: i64 = "text_color",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#giftinfo"] TelegramGiftInfo {
    gift: Box<TelegramGift> = "gift",
    owned_gift_id: String = "owned_gift_id",
    convert_star_count: i64 = "convert_star_count",
    prepaid_upgrade_star_count: i64 = "prepaid_upgrade_star_count",
    is_upgrade_separate: bool = "is_upgrade_separate",
    can_be_upgraded: bool = "can_be_upgraded",
    text: String = "text",
    entities: Vec<TelegramMessageEntity> = "entities",
    is_private: bool = "is_private",
    unique_gift_number: i64 = "unique_gift_number",
});

model!(#[doc = "https://core.telegram.org/bots/api#giveaway"] TelegramGiveaway {
    chats: Vec<TelegramChat> = "chats",
    winners_selection_date: i64 = "winners_selection_date",
    winner_count: i64 = "winner_count",
    only_new_members: bool = "only_new_members",
    has_public_winners: bool = "has_public_winners",
    prize_description: String = "prize_description",
    country_codes: Vec<String> = "country_codes",
    prize_star_count: i64 = "prize_star_count",
    premium_subscription_month_count: i64 = "premium_subscription_month_count",
});

model!(#[doc = "https://core.telegram.org/bots/api#giveawaycompleted"] TelegramGiveawayCompleted {
    winner_count: i64 = "winner_count",
    unclaimed_prize_count: i64 = "unclaimed_prize_count",
    giveaway_message: Box<TelegramMessage> = "giveaway_message",
    is_star_giveaway: bool = "is_star_giveaway",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#giveawaycreated"]
    TelegramGiveawayCreated {
        prize_star_count: i64 = "prize_star_count",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#giveawaywinners"] TelegramGiveawayWinners {
    chat: Box<TelegramChat> = "chat",
    giveaway_message_id: i64 = "giveaway_message_id",
    winners_selection_date: i64 = "winners_selection_date",
    winner_count: i64 = "winner_count",
    winners: Vec<TelegramUser> = "winners",
    additional_chat_count: i64 = "additional_chat_count",
    prize_star_count: i64 = "prize_star_count",
    premium_subscription_month_count: i64 = "premium_subscription_month_count",
    unclaimed_prize_count: i64 = "unclaimed_prize_count",
    only_new_members: bool = "only_new_members",
    was_refunded: bool = "was_refunded",
    prize_description: String = "prize_description",
});

model!(#[doc = "https://core.telegram.org/bots/api#inaccessiblemessage"] TelegramInaccessibleMessage {
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    date: i64 = "date",
});

model!(#[doc = "https://core.telegram.org/bots/api#inlinekeyboardbutton"] TelegramInlineKeyboardButton {
    text: String = "text",
    icon_custom_emoji_id: String = "icon_custom_emoji_id",
    style: String = "style",
    url: String = "url",
    callback_data: String = "callback_data",
    web_app: Box<TelegramWebAppInfo> = "web_app",
    login_url: Box<TelegramLoginUrl> = "login_url",
    switch_inline_query: String = "switch_inline_query",
    switch_inline_query_chosen_chat: Box<TelegramSwitchInlineQueryChosenChat> = "switch_inline_query_chosen_chat",
    copy_text: Box<TelegramCopyTextButton> = "copy_text",
    disabled: Box<TelegramDisabledButton> = "disabled",
});

model!(#[doc = "https://core.telegram.org/bots/api#inlinekeyboardmarkup"] TelegramInlineKeyboardMarkup {
    inline_keyboard: Vec<Vec<TelegramInlineKeyboardButton>> = "inline_keyboard",
    force_reply: bool = "force_reply",
});

model!(#[doc = "https://core.telegram.org/bots/api#inlinequery"] TelegramInlineQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    query: String = "query",
    offset: String = "offset",
    chat_type: String = "chat_type",
    location: Box<TelegramLocation> = "location",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#invoice"]
    TelegramInvoice {
        title: String = "title",
        description: String = "description",
        start_parameter: String = "start_parameter",
        currency: String = "currency",
        total_amount: i64 = "total_amount",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#link"]
    TelegramLink {
        url: String = "url",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#linkpreviewoptions"]
    TelegramLinkPreviewOptions {
        is_disabled: bool = "is_disabled",
        url: String = "url",
        prefer_small_media: bool = "prefer_small_media",
        prefer_large_media: bool = "prefer_large_media",
        show_above_text: bool = "show_above_text",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#livephoto"] TelegramLivePhoto {
    photo: Vec<TelegramPhotoSize> = "photo",
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    width: i64 = "width",
    height: i64 = "height",
    duration: i64 = "duration",
    mime_type: String = "mime_type",
    file_size: i64 = "file_size",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#location"]
    TelegramLocation {
        latitude: f64 = "latitude",
        longitude: f64 = "longitude",
        horizontal_accuracy: f64 = "horizontal_accuracy",
        live_period: i64 = "live_period",
        heading: i64 = "heading",
        proximity_alert_radius: i64 = "proximity_alert_radius",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#loginurl"]
    TelegramLoginUrl {
        forward_text: String = "forward_text",
        bot_username: String = "bot_username",
        request_write_access: bool = "request_write_access",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#managedbotcreated"] TelegramManagedBotCreated {
    bot: Box<TelegramUser> = "bot",
});

model!(#[doc = "https://core.telegram.org/bots/api#managedbotupdated"] TelegramManagedBotUpdated {
    user: Box<TelegramUser> = "user",
    bot: Box<TelegramUser> = "bot",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#maskposition"]
    TelegramMaskPosition {
        point: String = "point",
        x_shift: f64 = "x_shift",
        y_shift: f64 = "y_shift",
        scale: f64 = "scale",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#message"] TelegramMessage {
    message_id: i64 = "message_id",
    message_thread_id: i64 = "message_thread_id",
    direct_messages_topic: Box<TelegramDirectMessagesTopic> = "direct_messages_topic",
    from: Box<TelegramUser> = "from",
    sender_chat: Box<TelegramChat> = "sender_chat",
    sender_boost_count: i64 = "sender_boost_count",
    sender_business_bot: Box<TelegramUser> = "sender_business_bot",
    sender_tag: String = "sender_tag",
    receiver_user: Box<TelegramUser> = "receiver_user",
    ephemeral_message_id: i64 = "ephemeral_message_id",
    date: i64 = "date",
    guest_query_id: String = "guest_query_id",
    business_connection_id: String = "business_connection_id",
    chat: Box<TelegramChat> = "chat",
    forward_origin: Box<TelegramMessageOrigin> = "forward_origin",
    is_topic_message: bool = "is_topic_message",
    is_automatic_forward: bool = "is_automatic_forward",
    reply_to_message: Box<TelegramMessage> = "reply_to_message",
    external_reply: Box<TelegramExternalReplyInfo> = "external_reply",
    quote: Box<TelegramTextQuote> = "quote",
    reply_to_story: Box<TelegramStory> = "reply_to_story",
    reply_to_checklist_task_id: i64 = "reply_to_checklist_task_id",
    reply_to_poll_option_id: String = "reply_to_poll_option_id",
    via_bot: Box<TelegramUser> = "via_bot",
    guest_bot_caller_user: Box<TelegramUser> = "guest_bot_caller_user",
    guest_bot_caller_chat: Box<TelegramChat> = "guest_bot_caller_chat",
    edit_date: i64 = "edit_date",
    has_protected_content: bool = "has_protected_content",
    is_from_offline: bool = "is_from_offline",
    is_paid_post: bool = "is_paid_post",
    media_group_id: String = "media_group_id",
    author_signature: String = "author_signature",
    paid_star_count: i64 = "paid_star_count",
    text: String = "text",
    entities: Vec<TelegramMessageEntity> = "entities",
    link_preview_options: Box<TelegramLinkPreviewOptions> = "link_preview_options",
    suggested_post_info: Box<TelegramSuggestedPostInfo> = "suggested_post_info",
    effect_id: String = "effect_id",
    rich_message: Box<TelegramRichMessage> = "rich_message",
    animation: Box<TelegramAnimation> = "animation",
    audio: Box<TelegramAudio> = "audio",
    document: Box<TelegramDocument> = "document",
    live_photo: Box<TelegramLivePhoto> = "live_photo",
    paid_media: Box<TelegramPaidMediaInfo> = "paid_media",
    photo: Vec<TelegramPhotoSize> = "photo",
    sticker: Box<TelegramSticker> = "sticker",
    story: Box<TelegramStory> = "story",
    video: Box<TelegramVideo> = "video",
    video_note: Box<TelegramVideoNote> = "video_note",
    voice: Box<TelegramVoice> = "voice",
    widget: Box<TelegramWidget> = "widget",
    caption: String = "caption",
    caption_entities: Vec<TelegramMessageEntity> = "caption_entities",
    show_caption_above_media: bool = "show_caption_above_media",
    has_media_spoiler: bool = "has_media_spoiler",
    checklist: Box<TelegramChecklist> = "checklist",
    contact: Box<TelegramContact> = "contact",
    dice: Box<TelegramDice> = "dice",
    game: Box<TelegramGame> = "game",
    poll: Box<TelegramPoll> = "poll",
    venue: Box<TelegramVenue> = "venue",
    location: Box<TelegramLocation> = "location",
    new_chat_members: Vec<TelegramUser> = "new_chat_members",
    left_chat_member: Box<TelegramUser> = "left_chat_member",
    chat_owner_left: Box<TelegramChatOwnerLeft> = "chat_owner_left",
    chat_owner_changed: Box<TelegramChatOwnerChanged> = "chat_owner_changed",
    new_chat_title: String = "new_chat_title",
    new_chat_photo: Vec<TelegramPhotoSize> = "new_chat_photo",
    delete_chat_photo: bool = "delete_chat_photo",
    group_chat_created: bool = "group_chat_created",
    supergroup_chat_created: bool = "supergroup_chat_created",
    channel_chat_created: bool = "channel_chat_created",
    message_auto_delete_timer_changed: Box<TelegramMessageAutoDeleteTimerChanged> = "message_auto_delete_timer_changed",
    migrate_to_chat_id: i64 = "migrate_to_chat_id",
    migrate_from_chat_id: i64 = "migrate_from_chat_id",
    pinned_message: Box<TelegramMaybeInaccessibleMessage> = "pinned_message",
    invoice: Box<TelegramInvoice> = "invoice",
    successful_payment: Box<TelegramSuccessfulPayment> = "successful_payment",
    refunded_payment: Box<TelegramRefundedPayment> = "refunded_payment",
    users_shared: Box<TelegramUsersShared> = "users_shared",
    chat_shared: Box<TelegramChatShared> = "chat_shared",
    gift: Box<TelegramGiftInfo> = "gift",
    unique_gift: Box<TelegramUniqueGiftInfo> = "unique_gift",
    gift_upgrade_sent: Box<TelegramGiftInfo> = "gift_upgrade_sent",
    connected_website: String = "connected_website",
    write_access_allowed: Box<TelegramWriteAccessAllowed> = "write_access_allowed",
    passport_data: Box<TelegramPassportData> = "passport_data",
    proximity_alert_triggered: Box<TelegramProximityAlertTriggered> = "proximity_alert_triggered",
    boost_added: Box<TelegramChatBoostAdded> = "boost_added",
    chat_background_set: Box<TelegramChatBackground> = "chat_background_set",
    checklist_tasks_done: Box<TelegramChecklistTasksDone> = "checklist_tasks_done",
    checklist_tasks_added: Box<TelegramChecklistTasksAdded> = "checklist_tasks_added",
    community_chat_added: Box<TelegramCommunityChatAdded> = "community_chat_added",
    community_chat_joined: Box<TelegramCommunityChatJoined> = "community_chat_joined",
    community_chat_removed: Box<TelegramCommunityChatRemoved> = "community_chat_removed",
    direct_message_price_changed: Box<TelegramDirectMessagePriceChanged> = "direct_message_price_changed",
    forum_topic_created: Box<TelegramForumTopicCreated> = "forum_topic_created",
    forum_topic_edited: Box<TelegramForumTopicEdited> = "forum_topic_edited",
    forum_topic_closed: Box<TelegramForumTopicClosed> = "forum_topic_closed",
    forum_topic_reopened: Box<TelegramForumTopicReopened> = "forum_topic_reopened",
    general_forum_topic_hidden: Box<TelegramGeneralForumTopicHidden> = "general_forum_topic_hidden",
    general_forum_topic_unhidden: Box<TelegramGeneralForumTopicUnhidden> = "general_forum_topic_unhidden",
    giveaway_created: Box<TelegramGiveawayCreated> = "giveaway_created",
    giveaway: Box<TelegramGiveaway> = "giveaway",
    giveaway_winners: Box<TelegramGiveawayWinners> = "giveaway_winners",
    giveaway_completed: Box<TelegramGiveawayCompleted> = "giveaway_completed",
    managed_bot_created: Box<TelegramManagedBotCreated> = "managed_bot_created",
    paid_message_price_changed: Box<TelegramPaidMessagePriceChanged> = "paid_message_price_changed",
    paid_messages_waived: Box<TelegramPaidMessagesWaived> = "paid_messages_waived",
    poll_option_added: Box<TelegramPollOptionAdded> = "poll_option_added",
    poll_option_deleted: Box<TelegramPollOptionDeleted> = "poll_option_deleted",
    suggested_post_approved: Box<TelegramSuggestedPostApproved> = "suggested_post_approved",
    suggested_post_approval_failed: Box<TelegramSuggestedPostApprovalFailed> = "suggested_post_approval_failed",
    suggested_post_declined: Box<TelegramSuggestedPostDeclined> = "suggested_post_declined",
    suggested_post_paid: Box<TelegramSuggestedPostPaid> = "suggested_post_paid",
    suggested_post_refunded: Box<TelegramSuggestedPostRefunded> = "suggested_post_refunded",
    ton_wallet_transfer: Box<TelegramTonWalletTransfer> = "ton_wallet_transfer",
    video_chat_scheduled: Box<TelegramVideoChatScheduled> = "video_chat_scheduled",
    video_chat_started: Box<TelegramVideoChatStarted> = "video_chat_started",
    video_chat_ended: Box<TelegramVideoChatEnded> = "video_chat_ended",
    video_chat_participants_invited: Box<TelegramVideoChatParticipantsInvited> = "video_chat_participants_invited",
    web_app_data: Box<TelegramWebAppData> = "web_app_data",
    reply_markup: Box<TelegramInlineKeyboardMarkup> = "reply_markup",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#messageautodeletetimerchanged"]
    TelegramMessageAutoDeleteTimerChanged {
        message_auto_delete_time: i64 = "message_auto_delete_time",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#messageentity"] TelegramMessageEntity {
    r#type: String = "type",
    offset: i64 = "offset",
    length: i64 = "length",
    url: String = "url",
    user: Box<TelegramUser> = "user",
    language: String = "language",
    custom_emoji_id: String = "custom_emoji_id",
    unix_time: i64 = "unix_time",
    date_time_format: String = "date_time_format",
});

model!(#[doc = "https://core.telegram.org/bots/api#messagegenerationstopped"] TelegramMessageGenerationStopped {
    chat: Box<TelegramChat> = "chat",
    message_thread_id: i64 = "message_thread_id",
    draft_id: i64 = "draft_id",
});

union!(#[doc = "https://core.telegram.org/bots/api#messageorigin"] TelegramMessageOrigin, "type", {
    User(TelegramMessageOriginUser) = "user",
    HiddenUser(TelegramMessageOriginHiddenUser) = "hidden_user",
    Chat(TelegramMessageOriginChat) = "chat",
    Channel(TelegramMessageOriginChannel) = "channel",
});

model!(#[doc = "https://core.telegram.org/bots/api#messageoriginchannel"] TelegramMessageOriginChannel {
    r#type: String = "type",
    date: i64 = "date",
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    author_signature: String = "author_signature",
});

model!(#[doc = "https://core.telegram.org/bots/api#messageoriginchat"] TelegramMessageOriginChat {
    r#type: String = "type",
    date: i64 = "date",
    sender_chat: Box<TelegramChat> = "sender_chat",
    author_signature: String = "author_signature",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#messageoriginhiddenuser"]
    TelegramMessageOriginHiddenUser {
        r#type: String = "type",
        date: i64 = "date",
        sender_user_name: String = "sender_user_name",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#messageoriginuser"] TelegramMessageOriginUser {
    r#type: String = "type",
    date: i64 = "date",
    sender_user: Box<TelegramUser> = "sender_user",
});

model!(#[doc = "https://core.telegram.org/bots/api#messagereactioncountupdated"] TelegramMessageReactionCountUpdated {
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    date: i64 = "date",
    reactions: Vec<TelegramReactionCount> = "reactions",
});

model!(#[doc = "https://core.telegram.org/bots/api#messagereactionupdated"] TelegramMessageReactionUpdated {
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    user: Box<TelegramUser> = "user",
    actor_chat: Box<TelegramChat> = "actor_chat",
    date: i64 = "date",
    old_reaction: Vec<TelegramReactionType> = "old_reaction",
    new_reaction: Vec<TelegramReactionType> = "new_reaction",
});

model!(#[doc = "https://core.telegram.org/bots/api#orderinfo"] TelegramOrderInfo {
    name: String = "name",
    phone_number: String = "phone_number",
    email: String = "email",
    shipping_address: Box<TelegramShippingAddress> = "shipping_address",
});

union!(#[doc = "https://core.telegram.org/bots/api#paidmedia"] TelegramPaidMedia, "type", {
    LivePhoto(TelegramPaidMediaLivePhoto) = "live_photo",
    Photo(TelegramPaidMediaPhoto) = "photo",
    Preview(TelegramPaidMediaPreview) = "preview",
    Video(TelegramPaidMediaVideo) = "video",
});

model!(#[doc = "https://core.telegram.org/bots/api#paidmediainfo"] TelegramPaidMediaInfo {
    star_count: i64 = "star_count",
    paid_media: Vec<TelegramPaidMedia> = "paid_media",
});

model!(#[doc = "https://core.telegram.org/bots/api#paidmedialivephoto"] TelegramPaidMediaLivePhoto {
    r#type: String = "type",
    live_photo: Box<TelegramLivePhoto> = "live_photo",
});

model!(#[doc = "https://core.telegram.org/bots/api#paidmediaphoto"] TelegramPaidMediaPhoto {
    r#type: String = "type",
    photo: Vec<TelegramPhotoSize> = "photo",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#paidmediapreview"]
    TelegramPaidMediaPreview {
        r#type: String = "type",
        width: i64 = "width",
        height: i64 = "height",
        duration: i64 = "duration",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#paidmediapurchased"] TelegramPaidMediaPurchased {
    from: Box<TelegramUser> = "from",
    paid_media_payload: String = "paid_media_payload",
});

model!(#[doc = "https://core.telegram.org/bots/api#paidmediavideo"] TelegramPaidMediaVideo {
    r#type: String = "type",
    video: Box<TelegramVideo> = "video",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#paidmessagepricechanged"]
    TelegramPaidMessagePriceChanged {
        paid_message_star_count: i64 = "paid_message_star_count",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#paidmessageswaived"]
    TelegramPaidMessagesWaived {}
);

model!(#[doc = "https://core.telegram.org/bots/api#passportdata"] TelegramPassportData {
    data: Vec<TelegramEncryptedPassportElement> = "data",
    credentials: Box<TelegramEncryptedCredentials> = "credentials",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#passportfile"]
    TelegramPassportFile {
        file_id: String = "file_id",
        file_unique_id: String = "file_unique_id",
        file_size: i64 = "file_size",
        file_date: i64 = "file_date",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#photosize"]
    TelegramPhotoSize {
        file_id: String = "file_id",
        file_unique_id: String = "file_unique_id",
        width: i64 = "width",
        height: i64 = "height",
        file_size: i64 = "file_size",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#poll"] TelegramPoll {
    id: String = "id",
    question: String = "question",
    question_entities: Vec<TelegramMessageEntity> = "question_entities",
    options: Vec<TelegramPollOption> = "options",
    total_voter_count: i64 = "total_voter_count",
    is_closed: bool = "is_closed",
    is_anonymous: bool = "is_anonymous",
    r#type: String = "type",
    allows_multiple_answers: bool = "allows_multiple_answers",
    allows_revoting: bool = "allows_revoting",
    members_only: bool = "members_only",
    country_codes: Vec<String> = "country_codes",
    correct_option_ids: Vec<i64> = "correct_option_ids",
    explanation: String = "explanation",
    explanation_entities: Vec<TelegramMessageEntity> = "explanation_entities",
    explanation_media: Box<TelegramPollMedia> = "explanation_media",
    open_period: i64 = "open_period",
    close_date: i64 = "close_date",
    description: String = "description",
    description_entities: Vec<TelegramMessageEntity> = "description_entities",
    media: Box<TelegramPollMedia> = "media",
});

model!(#[doc = "https://core.telegram.org/bots/api#pollanswer"] TelegramPollAnswer {
    poll_id: String = "poll_id",
    voter_chat: Box<TelegramChat> = "voter_chat",
    user: Box<TelegramUser> = "user",
    option_ids: Vec<i64> = "option_ids",
    option_persistent_ids: Vec<String> = "option_persistent_ids",
});

model!(#[doc = "https://core.telegram.org/bots/api#pollmedia"] TelegramPollMedia {
    animation: Box<TelegramAnimation> = "animation",
    audio: Box<TelegramAudio> = "audio",
    document: Box<TelegramDocument> = "document",
    link: Box<TelegramLink> = "link",
    live_photo: Box<TelegramLivePhoto> = "live_photo",
    location: Box<TelegramLocation> = "location",
    photo: Vec<TelegramPhotoSize> = "photo",
    sticker: Box<TelegramSticker> = "sticker",
    venue: Box<TelegramVenue> = "venue",
    video: Box<TelegramVideo> = "video",
});

model!(#[doc = "https://core.telegram.org/bots/api#polloption"] TelegramPollOption {
    persistent_id: String = "persistent_id",
    text: String = "text",
    text_entities: Vec<TelegramMessageEntity> = "text_entities",
    media: Box<TelegramPollMedia> = "media",
    voter_count: i64 = "voter_count",
    added_by_user: Box<TelegramUser> = "added_by_user",
    added_by_chat: Box<TelegramChat> = "added_by_chat",
    addition_date: i64 = "addition_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#polloptionadded"] TelegramPollOptionAdded {
    poll_message: Box<TelegramMaybeInaccessibleMessage> = "poll_message",
    option_persistent_id: String = "option_persistent_id",
    option_text: String = "option_text",
    option_text_entities: Vec<TelegramMessageEntity> = "option_text_entities",
});

model!(#[doc = "https://core.telegram.org/bots/api#polloptiondeleted"] TelegramPollOptionDeleted {
    poll_message: Box<TelegramMaybeInaccessibleMessage> = "poll_message",
    option_persistent_id: String = "option_persistent_id",
    option_text: String = "option_text",
    option_text_entities: Vec<TelegramMessageEntity> = "option_text_entities",
});

model!(#[doc = "https://core.telegram.org/bots/api#precheckoutquery"] TelegramPreCheckoutQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    currency: String = "currency",
    total_amount: i64 = "total_amount",
    invoice_payload: String = "invoice_payload",
    shipping_option_id: String = "shipping_option_id",
    order_info: Box<TelegramOrderInfo> = "order_info",
});

model!(#[doc = "https://core.telegram.org/bots/api#proximityalerttriggered"] TelegramProximityAlertTriggered {
    traveler: Box<TelegramUser> = "traveler",
    watcher: Box<TelegramUser> = "watcher",
    distance: i64 = "distance",
});

model!(#[doc = "https://core.telegram.org/bots/api#reactioncount"] TelegramReactionCount {
    r#type: Box<TelegramReactionType> = "type",
    total_count: i64 = "total_count",
});

union!(#[doc = "https://core.telegram.org/bots/api#reactiontype"] TelegramReactionType, "type", {
    Emoji(TelegramReactionTypeEmoji) = "emoji",
    CustomEmoji(TelegramReactionTypeCustomEmoji) = "custom_emoji",
    Paid(TelegramReactionTypePaid) = "paid",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#reactiontypecustomemoji"]
    TelegramReactionTypeCustomEmoji {
        r#type: String = "type",
        custom_emoji_id: String = "custom_emoji_id",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#reactiontypeemoji"]
    TelegramReactionTypeEmoji {
        r#type: String = "type",
        emoji: String = "emoji",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#reactiontypepaid"]
    TelegramReactionTypePaid {
        r#type: String = "type",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#refundedpayment"]
    TelegramRefundedPayment {
        currency: String = "currency",
        total_amount: i64 = "total_amount",
        invoice_payload: String = "invoice_payload",
        telegram_payment_charge_id: String = "telegram_payment_charge_id",
        provider_payment_charge_id: String = "provider_payment_charge_id",
    }
);

union!(#[doc = "https://core.telegram.org/bots/api#richblock"] TelegramRichBlock, "type", {
    Paragraph(TelegramRichBlockParagraph) = "paragraph",
    SectionHeading(TelegramRichBlockSectionHeading) = "heading",
    Preformatted(TelegramRichBlockPreformatted) = "pre",
    Footer(TelegramRichBlockFooter) = "footer",
    Divider(TelegramRichBlockDivider) = "divider",
    MathematicalExpression(TelegramRichBlockMathematicalExpression) = "mathematical_expression",
    Anchor(TelegramRichBlockAnchor) = "anchor",
    List(TelegramRichBlockList) = "list",
    BlockQuotation(TelegramRichBlockBlockQuotation) = "blockquote",
    ExpandableBlockQuotation(TelegramRichBlockExpandableBlockQuotation) = "expandable_blockquote",
    PullQuotation(TelegramRichBlockPullQuotation) = "pullquote",
    Collage(TelegramRichBlockCollage) = "collage",
    Slideshow(TelegramRichBlockSlideshow) = "slideshow",
    Table(TelegramRichBlockTable) = "table",
    Details(TelegramRichBlockDetails) = "details",
    Map(TelegramRichBlockMap) = "map",
    Buttons(TelegramRichBlockButtons) = "buttons",
    Animation(TelegramRichBlockAnimation) = "animation",
    Audio(TelegramRichBlockAudio) = "audio",
    Document(TelegramRichBlockDocument) = "document",
    Photo(TelegramRichBlockPhoto) = "photo",
    Video(TelegramRichBlockVideo) = "video",
    VoiceNote(TelegramRichBlockVoiceNote) = "voice_note",
    Thinking(TelegramRichBlockThinking) = "thinking",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richblockanchor"]
    TelegramRichBlockAnchor {
        r#type: String = "type",
        name: String = "name",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richblockanimation"] TelegramRichBlockAnimation {
    r#type: String = "type",
    animation: Box<TelegramAnimation> = "animation",
    has_spoiler: bool = "has_spoiler",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockaudio"] TelegramRichBlockAudio {
    r#type: String = "type",
    audio: Box<TelegramAudio> = "audio",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockblockquotation"] TelegramRichBlockBlockQuotation {
    r#type: String = "type",
    blocks: Vec<TelegramRichBlock> = "blocks",
    credit: Box<TelegramRichText> = "credit",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockbuttons"] TelegramRichBlockButtons {
    r#type: String = "type",
    buttons: Vec<TelegramRichMessageButton> = "buttons",
    align: String = "align",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockcaption"] TelegramRichBlockCaption {
    text: Box<TelegramRichText> = "text",
    credit: Box<TelegramRichText> = "credit",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockcollage"] TelegramRichBlockCollage {
    r#type: String = "type",
    blocks: Vec<TelegramRichBlock> = "blocks",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockdetails"] TelegramRichBlockDetails {
    r#type: String = "type",
    summary: Box<TelegramRichText> = "summary",
    blocks: Vec<TelegramRichBlock> = "blocks",
    is_open: bool = "is_open",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richblockdivider"]
    TelegramRichBlockDivider {
        r#type: String = "type",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richblockdocument"] TelegramRichBlockDocument {
    r#type: String = "type",
    document: Box<TelegramDocument> = "document",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockexpandableblockquotation"] TelegramRichBlockExpandableBlockQuotation {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    credit: Box<TelegramRichText> = "credit",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockfooter"] TelegramRichBlockFooter {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblocklist"] TelegramRichBlockList {
    r#type: String = "type",
    items: Vec<TelegramRichBlockListItem> = "items",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblocklistitem"] TelegramRichBlockListItem {
    label: String = "label",
    blocks: Vec<TelegramRichBlock> = "blocks",
    has_checkbox: bool = "has_checkbox",
    is_checked: bool = "is_checked",
    value: i64 = "value",
    r#type: String = "type",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockmap"] TelegramRichBlockMap {
    r#type: String = "type",
    location: Box<TelegramLocation> = "location",
    zoom: i64 = "zoom",
    width: i64 = "width",
    height: i64 = "height",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richblockmathematicalexpression"]
    TelegramRichBlockMathematicalExpression {
        r#type: String = "type",
        expression: String = "expression",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richblockparagraph"] TelegramRichBlockParagraph {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockphoto"] TelegramRichBlockPhoto {
    r#type: String = "type",
    photo: Vec<TelegramPhotoSize> = "photo",
    has_spoiler: bool = "has_spoiler",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockpreformatted"] TelegramRichBlockPreformatted {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    language: String = "language",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockpullquotation"] TelegramRichBlockPullQuotation {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    credit: Box<TelegramRichText> = "credit",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblocksectionheading"] TelegramRichBlockSectionHeading {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    size: i64 = "size",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockslideshow"] TelegramRichBlockSlideshow {
    r#type: String = "type",
    blocks: Vec<TelegramRichBlock> = "blocks",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblocktable"] TelegramRichBlockTable {
    r#type: String = "type",
    cells: Vec<Vec<TelegramRichBlockTableCell>> = "cells",
    is_bordered: bool = "is_bordered",
    is_striped: bool = "is_striped",
    is_compact: bool = "is_compact",
    caption: Box<TelegramRichText> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblocktablecell"] TelegramRichBlockTableCell {
    text: Box<TelegramRichText> = "text",
    is_header: bool = "is_header",
    colspan: i64 = "colspan",
    rowspan: i64 = "rowspan",
    align: String = "align",
    valign: String = "valign",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockthinking"] TelegramRichBlockThinking {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockvideo"] TelegramRichBlockVideo {
    r#type: String = "type",
    video: Box<TelegramVideo> = "video",
    has_spoiler: bool = "has_spoiler",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richblockvoicenote"] TelegramRichBlockVoiceNote {
    r#type: String = "type",
    voice_note: Box<TelegramVoice> = "voice_note",
    caption: Box<TelegramRichBlockCaption> = "caption",
});

model!(#[doc = "https://core.telegram.org/bots/api#richmessage"] TelegramRichMessage {
    blocks: Vec<TelegramRichBlock> = "blocks",
    is_rtl: bool = "is_rtl",
});

model!(#[doc = "https://core.telegram.org/bots/api#richmessagebutton"] TelegramRichMessageButton {
    text: Box<TelegramRichText> = "text",
    style: String = "style",
    url: String = "url",
    callback_data: String = "callback_data",
    web_app: Box<TelegramWebAppInfo> = "web_app",
    login_url: Box<TelegramLoginUrl> = "login_url",
    switch_inline_query: String = "switch_inline_query",
    switch_inline_query_current_chat: String = "switch_inline_query_current_chat",
    switch_inline_query_chosen_chat: Box<TelegramSwitchInlineQueryChosenChat> = "switch_inline_query_chosen_chat",
    copy_text: Box<TelegramCopyTextButton> = "copy_text",
    disabled: Box<TelegramDisabledButton> = "disabled",
});

union!(#[doc = "https://core.telegram.org/bots/api#richtext"] TelegramFormattedRichText, "type", {
    Bold(TelegramRichTextBold) = "bold",
    Italic(TelegramRichTextItalic) = "italic",
    Underline(TelegramRichTextUnderline) = "underline",
    Strikethrough(TelegramRichTextStrikethrough) = "strikethrough",
    Spoiler(TelegramRichTextSpoiler) = "spoiler",
    DateTime(TelegramRichTextDateTime) = "date_time",
    TextMention(TelegramRichTextTextMention) = "text_mention",
    Subscript(TelegramRichTextSubscript) = "subscript",
    Superscript(TelegramRichTextSuperscript) = "superscript",
    Marked(TelegramRichTextMarked) = "marked",
    Code(TelegramRichTextCode) = "code",
    CustomEmoji(TelegramRichTextCustomEmoji) = "custom_emoji",
    MathematicalExpression(TelegramRichTextMathematicalExpression) = "mathematical_expression",
    Url(TelegramRichTextUrl) = "url",
    EmailAddress(TelegramRichTextEmailAddress) = "email_address",
    PhoneNumber(TelegramRichTextPhoneNumber) = "phone_number",
    BankCardNumber(TelegramRichTextBankCardNumber) = "bank_card_number",
    TonAddress(TelegramRichTextTonAddress) = "ton_address",
    Mention(TelegramRichTextMention) = "mention",
    Hashtag(TelegramRichTextHashtag) = "hashtag",
    Cashtag(TelegramRichTextCashtag) = "cashtag",
    BotCommand(TelegramRichTextBotCommand) = "bot_command",
    Button(TelegramRichTextButton) = "button",
    Anchor(TelegramRichTextAnchor) = "anchor",
    AnchorLink(TelegramRichTextAnchorLink) = "anchor_link",
    Reference(TelegramRichTextReference) = "reference",
    ReferenceLink(TelegramRichTextReferenceLink) = "reference_link",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richtextanchor"]
    TelegramRichTextAnchor {
        r#type: String = "type",
        name: String = "name",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richtextanchorlink"] TelegramRichTextAnchorLink {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    anchor_name: String = "anchor_name",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextbankcardnumber"] TelegramRichTextBankCardNumber {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    bank_card_number: String = "bank_card_number",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextbold"] TelegramRichTextBold {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextbotcommand"] TelegramRichTextBotCommand {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    bot_command: String = "bot_command",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextbutton"] TelegramRichTextButton {
    r#type: String = "type",
    button: Box<TelegramRichMessageButton> = "button",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextcashtag"] TelegramRichTextCashtag {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    cashtag: String = "cashtag",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextcode"] TelegramRichTextCode {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richtextcustomemoji"]
    TelegramRichTextCustomEmoji {
        r#type: String = "type",
        custom_emoji_id: String = "custom_emoji_id",
        alternative_text: String = "alternative_text",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richtextdatetime"] TelegramRichTextDateTime {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    unix_time: i64 = "unix_time",
    date_time_format: String = "date_time_format",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextemailaddress"] TelegramRichTextEmailAddress {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    email_address: String = "email_address",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtexthashtag"] TelegramRichTextHashtag {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    hashtag: String = "hashtag",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextitalic"] TelegramRichTextItalic {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextmarked"] TelegramRichTextMarked {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#richtextmathematicalexpression"]
    TelegramRichTextMathematicalExpression {
        r#type: String = "type",
        expression: String = "expression",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#richtextmention"] TelegramRichTextMention {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    username: String = "username",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextphonenumber"] TelegramRichTextPhoneNumber {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    phone_number: String = "phone_number",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextreference"] TelegramRichTextReference {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    name: String = "name",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextreferencelink"] TelegramRichTextReferenceLink {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    reference_name: String = "reference_name",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextspoiler"] TelegramRichTextSpoiler {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextstrikethrough"] TelegramRichTextStrikethrough {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextsubscript"] TelegramRichTextSubscript {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextsuperscript"] TelegramRichTextSuperscript {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtexttextmention"] TelegramRichTextTextMention {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    user: Box<TelegramUser> = "user",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtexttonaddress"] TelegramRichTextTonAddress {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    address: String = "address",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtextunderline"] TelegramRichTextUnderline {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
});

model!(#[doc = "https://core.telegram.org/bots/api#richtexturl"] TelegramRichTextUrl {
    r#type: String = "type",
    text: Box<TelegramRichText> = "text",
    url: String = "url",
});

model!(#[doc = "https://core.telegram.org/bots/api#shareduser"] TelegramSharedUser {
    user_id: i64 = "user_id",
    first_name: String = "first_name",
    last_name: String = "last_name",
    username: String = "username",
    photo: Vec<TelegramPhotoSize> = "photo",
});

model!(#[doc = "https://core.telegram.org/bots/api#sharedwidgetquery"] TelegramSharedWidgetQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    r#type: Box<TelegramWidgetQueryType> = "type",
    data: String = "data",
    widget_id: String = "widget_id",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#shippingaddress"]
    TelegramShippingAddress {
        country_code: String = "country_code",
        state: String = "state",
        city: String = "city",
        street_line1: String = "street_line1",
        street_line2: String = "street_line2",
        post_code: String = "post_code",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#shippingquery"] TelegramShippingQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    invoice_payload: String = "invoice_payload",
    shipping_address: Box<TelegramShippingAddress> = "shipping_address",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#staramount"]
    TelegramStarAmount {
        amount: i64 = "amount",
        nanostar_amount: i64 = "nanostar_amount",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#sticker"] TelegramSticker {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    r#type: String = "type",
    width: i64 = "width",
    height: i64 = "height",
    is_animated: bool = "is_animated",
    is_video: bool = "is_video",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
    emoji: String = "emoji",
    set_name: String = "set_name",
    premium_animation: Box<TelegramFile> = "premium_animation",
    mask_position: Box<TelegramMaskPosition> = "mask_position",
    custom_emoji_id: String = "custom_emoji_id",
    needs_repainting: bool = "needs_repainting",
    file_size: i64 = "file_size",
});

model!(#[doc = "https://core.telegram.org/bots/api#story"] TelegramStory {
    chat: Box<TelegramChat> = "chat",
    id: i64 = "id",
});

model!(#[doc = "https://core.telegram.org/bots/api#successfulpayment"] TelegramSuccessfulPayment {
    currency: String = "currency",
    total_amount: i64 = "total_amount",
    invoice_payload: String = "invoice_payload",
    subscription_expiration_date: i64 = "subscription_expiration_date",
    is_recurring: bool = "is_recurring",
    is_first_recurring: bool = "is_first_recurring",
    shipping_option_id: String = "shipping_option_id",
    order_info: Box<TelegramOrderInfo> = "order_info",
    telegram_payment_charge_id: String = "telegram_payment_charge_id",
    provider_payment_charge_id: String = "provider_payment_charge_id",
});

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostapprovalfailed"] TelegramSuggestedPostApprovalFailed {
    suggested_post_message: Box<TelegramMessage> = "suggested_post_message",
    price: Box<TelegramSuggestedPostPrice> = "price",
});

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostapproved"] TelegramSuggestedPostApproved {
    suggested_post_message: Box<TelegramMessage> = "suggested_post_message",
    price: Box<TelegramSuggestedPostPrice> = "price",
    send_date: i64 = "send_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostdeclined"] TelegramSuggestedPostDeclined {
    suggested_post_message: Box<TelegramMessage> = "suggested_post_message",
    comment: String = "comment",
});

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostinfo"] TelegramSuggestedPostInfo {
    state: String = "state",
    price: Box<TelegramSuggestedPostPrice> = "price",
    send_date: i64 = "send_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostpaid"] TelegramSuggestedPostPaid {
    suggested_post_message: Box<TelegramMessage> = "suggested_post_message",
    currency: String = "currency",
    amount: i64 = "amount",
    star_amount: Box<TelegramStarAmount> = "star_amount",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#suggestedpostprice"]
    TelegramSuggestedPostPrice {
        currency: String = "currency",
        amount: i64 = "amount",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#suggestedpostrefunded"] TelegramSuggestedPostRefunded {
    suggested_post_message: Box<TelegramMessage> = "suggested_post_message",
    reason: String = "reason",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#switchinlinequerychosenchat"]
    TelegramSwitchInlineQueryChosenChat {
        query: String = "query",
        allow_user_chats: bool = "allow_user_chats",
        allow_bot_chats: bool = "allow_bot_chats",
        allow_group_chats: bool = "allow_group_chats",
        allow_channel_chats: bool = "allow_channel_chats",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#textquote"] TelegramTextQuote {
    text: String = "text",
    entities: Vec<TelegramMessageEntity> = "entities",
    rich_texts: Vec<TelegramRichText> = "rich_texts",
    position: i64 = "position",
    is_manual: bool = "is_manual",
});

model!(#[doc = "https://core.telegram.org/bots/api#tonnft"] TelegramTonNft {
    address: String = "address",
    collection_address: String = "collection_address",
    index: String = "index",
    name: String = "name",
    description: String = "description",
    attributes: Vec<TelegramTonNftAttribute> = "attributes",
    gift: Box<TelegramUniqueGift> = "gift",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#tonnftattribute"]
    TelegramTonNftAttribute {
        trait_type: String = "trait_type",
        value: String = "value",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#tonwalletcard"] TelegramTonWalletCard {
    address: String = "address",
    amount: String = "amount",
    stickers: Vec<TelegramTonWalletCardSticker> = "stickers",
    background_nft_address: String = "background_nft_address",
});

model!(#[doc = "https://core.telegram.org/bots/api#tonwalletcardsticker"] TelegramTonWalletCardSticker {
    position: Box<TelegramAreaPosition> = "position",
    nft: Box<TelegramTonNft> = "nft",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#tonwallettransfer"]
    TelegramTonWalletTransfer {
        transaction_id: String = "transaction_id",
        peer_address: String = "peer_address",
        amount: String = "amount",
        comment: String = "comment",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#uniquegift"] TelegramUniqueGift {
    gift_id: String = "gift_id",
    base_name: String = "base_name",
    name: String = "name",
    number: i64 = "number",
    model: Box<TelegramUniqueGiftModel> = "model",
    symbol: Box<TelegramUniqueGiftSymbol> = "symbol",
    backdrop: Box<TelegramUniqueGiftBackdrop> = "backdrop",
    is_premium: bool = "is_premium",
    is_burned: bool = "is_burned",
    is_from_blockchain: bool = "is_from_blockchain",
    colors: Box<TelegramUniqueGiftColors> = "colors",
    publisher_chat: Box<TelegramChat> = "publisher_chat",
});

model!(#[doc = "https://core.telegram.org/bots/api#uniquegiftbackdrop"] TelegramUniqueGiftBackdrop {
    name: String = "name",
    colors: Box<TelegramUniqueGiftBackdropColors> = "colors",
    rarity_per_mille: i64 = "rarity_per_mille",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#uniquegiftbackdropcolors"]
    TelegramUniqueGiftBackdropColors {
        center_color: i64 = "center_color",
        edge_color: i64 = "edge_color",
        symbol_color: i64 = "symbol_color",
        text_color: i64 = "text_color",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#uniquegiftcolors"] TelegramUniqueGiftColors {
    model_custom_emoji_id: String = "model_custom_emoji_id",
    symbol_custom_emoji_id: String = "symbol_custom_emoji_id",
    light_theme_main_color: i64 = "light_theme_main_color",
    light_theme_other_colors: Vec<i64> = "light_theme_other_colors",
    dark_theme_main_color: i64 = "dark_theme_main_color",
    dark_theme_other_colors: Vec<i64> = "dark_theme_other_colors",
});

model!(#[doc = "https://core.telegram.org/bots/api#uniquegiftinfo"] TelegramUniqueGiftInfo {
    gift: Box<TelegramUniqueGift> = "gift",
    origin: String = "origin",
    text: String = "text",
    entities: Vec<TelegramMessageEntity> = "entities",
    is_private: bool = "is_private",
    last_resale_currency: String = "last_resale_currency",
    last_resale_amount: i64 = "last_resale_amount",
    owned_gift_id: String = "owned_gift_id",
    transfer_star_count: i64 = "transfer_star_count",
    next_transfer_date: i64 = "next_transfer_date",
});

model!(#[doc = "https://core.telegram.org/bots/api#uniquegiftmodel"] TelegramUniqueGiftModel {
    name: String = "name",
    sticker: Box<TelegramSticker> = "sticker",
    rarity_per_mille: i64 = "rarity_per_mille",
    rarity: String = "rarity",
});

model!(#[doc = "https://core.telegram.org/bots/api#uniquegiftsymbol"] TelegramUniqueGiftSymbol {
    name: String = "name",
    sticker: Box<TelegramSticker> = "sticker",
    rarity_per_mille: i64 = "rarity_per_mille",
});

model!(#[doc = "https://core.telegram.org/bots/api#update"] TelegramWebhookPayload [update_id: i64] {
    message: Box<TelegramMessage> = "message",
    edited_message: Box<TelegramMessage> = "edited_message",
    channel_post: Box<TelegramMessage> = "channel_post",
    edited_channel_post: Box<TelegramMessage> = "edited_channel_post",
    business_connection: Box<TelegramBusinessConnection> = "business_connection",
    business_message: Box<TelegramMessage> = "business_message",
    edited_business_message: Box<TelegramMessage> = "edited_business_message",
    deleted_business_messages: Box<TelegramBusinessMessagesDeleted> = "deleted_business_messages",
    guest_message: Box<TelegramMessage> = "guest_message",
    message_reaction: Box<TelegramMessageReactionUpdated> = "message_reaction",
    message_reaction_count: Box<TelegramMessageReactionCountUpdated> = "message_reaction_count",
    inline_query: Box<TelegramInlineQuery> = "inline_query",
    chosen_inline_result: Box<TelegramChosenInlineResult> = "chosen_inline_result",
    callback_query: Box<TelegramCallbackQuery> = "callback_query",
    widget_query: Box<TelegramWidgetQuery> = "widget_query",
    shared_widget_query: Box<TelegramSharedWidgetQuery> = "shared_widget_query",
    shipping_query: Box<TelegramShippingQuery> = "shipping_query",
    pre_checkout_query: Box<TelegramPreCheckoutQuery> = "pre_checkout_query",
    purchased_paid_media: Box<TelegramPaidMediaPurchased> = "purchased_paid_media",
    poll: Box<TelegramPoll> = "poll",
    poll_answer: Box<TelegramPollAnswer> = "poll_answer",
    my_chat_member: Box<TelegramChatMemberUpdated> = "my_chat_member",
    chat_member: Box<TelegramChatMemberUpdated> = "chat_member",
    chat_join_request: Box<TelegramChatJoinRequest> = "chat_join_request",
    chat_boost: Box<TelegramChatBoostUpdated> = "chat_boost",
    removed_chat_boost: Box<TelegramChatBoostRemoved> = "removed_chat_boost",
    managed_bot: Box<TelegramManagedBotUpdated> = "managed_bot",
    subscription: Box<TelegramBotSubscriptionUpdated> = "subscription",
    stopped_message_generation: Box<TelegramMessageGenerationStopped> = "stopped_message_generation",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#user"]
    TelegramUser {
        id: i64 = "id",
        is_bot: bool = "is_bot",
        first_name: String = "first_name",
        last_name: String = "last_name",
        username: String = "username",
        language_code: String = "language_code",
        is_premium: bool = "is_premium",
        added_to_attachment_menu: bool = "added_to_attachment_menu",
        can_join_groups: bool = "can_join_groups",
        can_read_all_group_messages: bool = "can_read_all_group_messages",
        supports_guest_queries: bool = "supports_guest_queries",
        supports_inline_queries: bool = "supports_inline_queries",
        can_connect_to_business: bool = "can_connect_to_business",
        has_main_web_app: bool = "has_main_web_app",
        has_topics_enabled: bool = "has_topics_enabled",
        allows_users_to_create_topics: bool = "allows_users_to_create_topics",
        can_manage_bots: bool = "can_manage_bots",
        supports_join_request_queries: bool = "supports_join_request_queries",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#usersshared"] TelegramUsersShared {
    request_id: i64 = "request_id",
    users: Vec<TelegramSharedUser> = "users",
});

model!(#[doc = "https://core.telegram.org/bots/api#venue"] TelegramVenue {
    location: Box<TelegramLocation> = "location",
    title: String = "title",
    address: String = "address",
    foursquare_id: String = "foursquare_id",
    foursquare_type: String = "foursquare_type",
    google_place_id: String = "google_place_id",
    google_place_type: String = "google_place_type",
});

model!(#[doc = "https://core.telegram.org/bots/api#video"] TelegramVideo {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    width: i64 = "width",
    height: i64 = "height",
    duration: i64 = "duration",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
    cover: Vec<TelegramPhotoSize> = "cover",
    start_timestamp: i64 = "start_timestamp",
    qualities: Vec<TelegramVideoQuality> = "qualities",
    file_name: String = "file_name",
    mime_type: String = "mime_type",
    file_size: i64 = "file_size",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#videochatended"]
    TelegramVideoChatEnded {
        duration: i64 = "duration",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#videochatparticipantsinvited"] TelegramVideoChatParticipantsInvited {
    users: Vec<TelegramUser> = "users",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#videochatscheduled"]
    TelegramVideoChatScheduled {
        start_date: i64 = "start_date",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#videochatstarted"]
    TelegramVideoChatStarted {}
);

model!(#[doc = "https://core.telegram.org/bots/api#videonote"] TelegramVideoNote {
    file_id: String = "file_id",
    file_unique_id: String = "file_unique_id",
    length: i64 = "length",
    duration: i64 = "duration",
    thumbnail: Box<TelegramPhotoSize> = "thumbnail",
    file_size: i64 = "file_size",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#videoquality"]
    TelegramVideoQuality {
        file_id: String = "file_id",
        file_unique_id: String = "file_unique_id",
        width: i64 = "width",
        height: i64 = "height",
        codec: String = "codec",
        file_size: i64 = "file_size",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#voice"]
    TelegramVoice {
        file_id: String = "file_id",
        file_unique_id: String = "file_unique_id",
        duration: i64 = "duration",
        mime_type: String = "mime_type",
        file_size: i64 = "file_size",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#webappdata"]
    TelegramWebAppData {
        data: String = "data",
        button_text: String = "button_text",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#webappinfo"]
    TelegramWebAppInfo {
        url: String = "url",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#widget"]
    TelegramWidget {
        id: String = "id",
        width: i64 = "width",
        height: i64 = "height",
        html: String = "html",
        public_data: String = "public_data",
        chat_data: String = "chat_data",
    }
);

model!(#[doc = "https://core.telegram.org/bots/api#widgetquery"] TelegramWidgetQuery {
    id: String = "id",
    from: Box<TelegramUser> = "from",
    chat: Box<TelegramChat> = "chat",
    message_id: i64 = "message_id",
    inline_message_id: String = "inline_message_id",
    chat_instance: String = "chat_instance",
    chat_type: String = "chat_type",
    r#type: Box<TelegramWidgetQueryType> = "type",
    data: String = "data",
    widget_id: String = "widget_id",
});

union!(#[doc = "https://core.telegram.org/bots/api#widgetquerytype"] TelegramWidgetQueryType, "type", {
    Data(TelegramWidgetQueryTypeData) = "data",
    SignData(TelegramWidgetQueryTypeSignData) = "sign_data",
    SentTransaction(TelegramWidgetQueryTypeSentTransaction) = "sent_transaction",
});

model!(#[doc = "https://core.telegram.org/bots/api#widgetquerytypedata"] TelegramWidgetQueryTypeData {
    r#type: String = "type",
    wallet_card: Box<TelegramTonWalletCard> = "wallet_card",
});

model!(#[doc = "https://core.telegram.org/bots/api#widgetquerytypesenttransaction"] TelegramWidgetQueryTypeSentTransaction {
    r#type: String = "type",
    wallet: Box<TelegramWidgetQueryWallet> = "wallet",
    msg_hash: String = "msg_hash",
});

model!(#[doc = "https://core.telegram.org/bots/api#widgetquerytypesigndata"] TelegramWidgetQueryTypeSignData {
    r#type: String = "type",
    wallet: Box<TelegramWidgetQueryWallet> = "wallet",
    sign_data: String = "sign_data",
});

model!(
    #[doc = "https://core.telegram.org/bots/api#widgetquerywallet"]
    TelegramWidgetQueryWallet {
        address: String = "address",
        amount: String = "amount",
        public_key: String = "public_key",
        state_init: String = "state_init",
    }
);

model!(
    #[doc = "https://core.telegram.org/bots/api#writeaccessallowed"]
    TelegramWriteAccessAllowed {
        from_request: bool = "from_request",
        web_app_name: String = "web_app_name",
        from_attachment_menu: bool = "from_attachment_menu",
    }
);

pub(super) const UPDATE_FIELDS: &[&str] = &[
    "message",
    "edited_message",
    "channel_post",
    "edited_channel_post",
    "business_connection",
    "business_message",
    "edited_business_message",
    "deleted_business_messages",
    "guest_message",
    "message_reaction",
    "message_reaction_count",
    "inline_query",
    "chosen_inline_result",
    "callback_query",
    "widget_query",
    "shared_widget_query",
    "shipping_query",
    "pre_checkout_query",
    "purchased_paid_media",
    "poll",
    "poll_answer",
    "my_chat_member",
    "chat_member",
    "chat_join_request",
    "chat_boost",
    "removed_chat_boost",
    "managed_bot",
    "subscription",
    "stopped_message_generation",
];
