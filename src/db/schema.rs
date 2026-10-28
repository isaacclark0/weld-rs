diesel::table! {
    invitations (id) {
        id -> Bigint,
        email -> Text,
        token_hash -> Text,
        invited_by -> Bigint,
        expires_at -> Timestamp,
        accepted_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
    }
}
