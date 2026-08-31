create table if not exists pastes (
  id text primary key,
  content text not null,
  format text not null,
  encrypted boolean not null default false,
  algorithm text,
  salt text,
  nonce text,
  burn_after_reading boolean not null default false,
  burned boolean not null default false,
  retention_minutes integer,
  expires_at timestamptz,
  created_at timestamptz not null default now(),
  view_count integer not null default 0
);

create index if not exists pastes_expires_at_idx on pastes (expires_at);
