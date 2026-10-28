diesel::table! {
    users (id) {
        id -> Bigint,
        email -> Text,
        name -> Nullable<Text>,
        password_hash -> Nullable<Text>,
        email_verified_at -> Nullable<Timestamp>,
        image -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    sessions (id) {
        id -> Bigint,
        token_hash -> Text,
        user_id -> Bigint,
        expires_at -> Timestamp,
        ip_address -> Nullable<Text>,
        user_agent -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    verifications (id) {
        id -> Bigint,
        identifier -> Text,
        token_hash -> Text,
        expires_at -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    accounts (id) {
        id -> Bigint,
        user_id -> Bigint,
        provider_id -> Text,
        account_id -> Text,
        access_token -> Nullable<Text>,
        refresh_token -> Nullable<Text>,
        access_token_expires_at -> Nullable<Timestamp>,
        scope -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    oauth_states (id) {
        id -> Bigint,
        provider_id -> Text,
        csrf_state -> Text,
        pkce_verifier -> Text,
        intent -> Text,
        link_user_id -> Nullable<Bigint>,
        expires_at -> Timestamp,
        created_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    users,
    sessions,
    verifications,
    accounts,
    oauth_states
);
