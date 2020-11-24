<template>
  <v-row justify="center" align="center">
    <v-col cols="12" sm="8" md="6">
      <p>Redirecting...</p>
    </v-col>
  </v-row>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'

@Component
class Logout extends Vue {
  async mounted () {
    const sleep = (msec: number) => new Promise(resolve => setTimeout(resolve, msec))
    this.$cookies.remove('access_token')
    this.$cookies.remove('refresh_token')
    this.$cookies.remove('user')
    this.$cookies.remove('session')
    this.$store.dispatch('auth/setAccessToken', '')
    this.$store.dispatch('auth/setRefreshToken', '')
    this.$store.dispatch('auth/setUser', null)
    await sleep(500)
    this.$router.push('/')
  }
}
export default Logout
</script>
