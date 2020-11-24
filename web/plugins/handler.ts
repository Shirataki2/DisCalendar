/* eslint-disable camelcase */
import { NuxtAxiosInstance } from '@nuxtjs/axios'
const JSONb = require('json-bigint')

export namespace Discord {
  export interface PartialGuild {
    id: bigint
    name: string
    icon?: string
    owner: boolean
    features: Array<string>
    permissions_new: string
    permissions: number
  }

  export interface Role {
    id: bigint
    name: string
    color: number
    hoist: boolean
    position: number
    permissions: string
    managed: boolean
    mentionable: boolean
  }

  export interface Emoji {
    id?: bigint
    name?: string
    roles?: Array<string>
    user?: User
    require_colons?: boolean
    managed?: boolean
    animated?: boolean
    available?: boolean
  }

  export interface User {
    id: bigint
    username: string
    discriminator: string
    avatar: string | null
    bot?: boolean
    system?: boolean
    mfa_enabled?: boolean
    locale?: string
    verified?: boolean
    email?: string | null
    flags?: number
    premium_type?: number
    public_flags?: number
  }

  export interface Member {
    user?: User
    nick?: string
    roles: Array<bigint>
    joined_at: Date
    premium_since?: Date | null
    deaf: boolean
    mute: boolean
  }

  export interface Guild extends PartialGuild {
    splash?: string
    discovery_splash?: string
    owner_id: bigint
    region: string
    afk_channel_id: bigint
    afk_timeout: number
    widget_enabled?: boolean
    widget_channel_id?: bigint
    verification_level: 0 | 1 | 2 | 3 | 4
    default_message_notifications: number
    explicit_content_filter: number
    roles: Array<Role>
    emojis: Array<Emoji>
    mfa_level: number
    application_id?: bigint
    system_channel_id?: bigint
    system_channel_flags: boolean
    rules_channel_id?: bigint
    joined_at?: Date
    large?: boolean
    unavailable?: boolean
    member_count?: number
    voice_states?: Array<any>
    members?: Array<Member>
    channels?: Array<any>
    presences?: Array<any>
    max_presences?: number
    max_members?: number
    vanity_url_code?: string
    description: string | null
    banner: string | null
    premium_tier: 0 | 1 | 2 | 3
    premium_subscription_count?: number
    preferred_locale: string
    public_updates_channel_id?: bigint
    max_video_channel_users?: number
    approximate_member_count?: number
    approximate_presence_count?: number
  }

  export interface MinimalGuild {
    id: bigint
    name: string
    guild_id: bigint
    avatar_url: string
  }

  export interface CheckResult {
    guild: Guild
    result: Array<string>
  }

  export interface RoleCheckResult {
    initialized: boolean
    authenticated: boolean
    authorized: boolean
    roles: Array<string>
    guild?: Guild
  }
}

export interface DiscordAPIInterface {
  getServerList: (
    axios: NuxtAxiosInstance
  ) => Promise<Array<Discord.PartialGuild>>

  getBotJoinedServerList: (
    axios: NuxtAxiosInstance,
    guild_ids: Array<bigint>
  ) => Promise<Array<Discord.MinimalGuild>>

  getServer: (
    axios: NuxtAxiosInstance,
    guild_id: string
  ) => Promise<Discord.Guild>
}

export class DiscordAPI implements DiscordAPIInterface {
  getServerList = async (axios: NuxtAxiosInstance) => {
    const { data } = await axios.get<Array<Discord.PartialGuild>>(
      '/discord/api/users/@me/guilds'
    )
    return data
  }

  getBotJoinedServerList = async (
    axios: NuxtAxiosInstance,
    guild_ids: Array<BigInt>
  ) => {
    const { data } = await axios.get<Array<Discord.MinimalGuild>>(
      '/local/api/guilds/joined',
      {
        params: {
          guild_ids: guild_ids.join(',')
        },
        transformResponse: [(data: any) => JSONb.parse(data)]
      }
    )
    return data
  }

  getServer = async (axios: NuxtAxiosInstance, guild_id: string) => {
    const { data } = await axios.get<Discord.Guild>(
      `/local/api/guilds/check/${guild_id}`
    )
    return data
  }
}
