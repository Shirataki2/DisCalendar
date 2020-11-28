<template>
  <v-footer app>
    <span>v 0.8.0</span>
    <v-spacer />
    <span>&copy; 2020</span>
  </v-footer>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'

@Component
class AppFooter extends Vue {
}
export default AppFooter
</script>
