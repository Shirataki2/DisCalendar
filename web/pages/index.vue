<template>
  <v-row justify="center" align="center">
    <v-col cols="12" sm="8" md="6">
      <h1>Home Page</h1>
      <v-btn color="info" to="/login">
        LOGIN
      </v-btn>
      <v-btn color="success" to="/authorized">
        AUTHORIZED
      </v-btn>
    </v-col>
  </v-row>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'

@Component({
  asyncData: async ({ store, app, $axios }) => {
    if (app.$cookies.get('access_token')) {
    }
    if (store.getters['auth/accessToken'] && store.getters['auth/user']) {
      return { userdata: store.getters['auth/user'] }
    } else if (app.$cookies.get('access_token')) {
      store.dispatch('auth/setAccessToken', app.$cookies.get('access_token'))
      store.dispatch('auth/setRefreshToken', app.$cookies.get('refresh_token'))
      try {
        const { data } = await $axios.get('/discord/api/users/@me')
        store.dispatch('auth/setUser', data)
        return { userdata: data }
      } catch (e) {
      }
    }
  }
})
class Index extends Vue {
}
export default Index
</script>
