<template>
  <v-footer id="footer" app outlined>
    <span>v 1.0.1</span>
    <v-spacer />
    <span>&copy; 2020</span>
  </v-footer>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'

@Component
class AppFooter extends Vue {
  get color () {
    if (this.$vuetify.theme.dark) {
      return 'grey darken-4'
    } else {
      return '#f7f7f7'
    }
  }
}
export default AppFooter
</script>

<style lang="scss" scoped>
.theme--light.v-footer {
    background-color: #fff;
    color: rgba(0, 0, 0, 0.87);
}
</style>
