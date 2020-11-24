<template>
  <p>callback</p>
</template>

<script lang="ts">
import queryString from 'querystring'
import { Component, Vue } from 'nuxt-property-decorator'

@Component({
  asyncData: async ({ store, query, $axios, app }) => {
    const code = query.code
    if (typeof code !== 'string') { return }
    const payload = {
      client_id: process.env.CLIENT_ID!,
      client_secret: process.env.CLIENT_SECRET!,
      grant_type: 'authorization_code',
      redirect_uri: process.env.REDIRECT_URI!,
      scope: 'identify guilds',
      code
    }
    const { data } = await $axios.post(
      '/discord/api/oauth2/token',
      queryString.stringify(payload),
      {
        headers: { 'Content-Type': 'application/x-www-form-urlencoded' }
      }
    )
    app.$cookies.set('access_token', data.access_token)
    app.$cookies.set('refresh_token', data.refresh_token)
    await store.dispatch('auth/setAccessToken', app.$cookies.get('access_token'))
    await store.dispatch('auth/setRefreshToken', app.$cookies.get('refresh_token'))
    const user = await $axios.get(
      '/discord/api/users/@me',
      {
        headers: {
          Authorization: 'Bearer ' + data.access_token
        }
      }
    )
    await store.dispatch('auth/setUser', user.data)
  }
})
class Callback extends Vue {
  mounted () {
    this.$router.push('/')
  }
}
export default Callback
</script>
