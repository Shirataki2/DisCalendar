<template>
  <v-app dark>
    <AppHeader />
    <v-main>
      <v-container>
        <nuxt />
      </v-container>
    </v-main>
    <AppFooter />
  </v-app>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'
import AppHeader from '@/components/header/AppHeader.vue'
import AppFooter from '@/components/AppFooter.vue'

@Component({
  components: { AppHeader, AppFooter }
})
class DefaultLayout extends Vue {
}
export default DefaultLayout
</script>
