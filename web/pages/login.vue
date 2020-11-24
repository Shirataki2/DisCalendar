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
class Index extends Vue {
  mounted () {
    location.href = process.env.OAUTH2_URL!
  }
}
export default Index
</script>
