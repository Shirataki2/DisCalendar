<template>
  <v-app-bar elevate-on-scroll app>
    <v-app-bar-nav-icon />
    <v-toolbar-title style="cursor: pointer" @click="$router.push('/')">
      <span id="logo">DisCalendar</span>
    </v-toolbar-title>
    <v-spacer />
    <div v-if="$route.path.match(/\/dashboard\/\d{18,}/g)">
      <CalendarController />
    </div>
    <v-spacer />
    <div v-if="$store.getters['auth/user']">
      <AccountMenu>
        <UserAvatar
          :avatar-id="$store.getters['auth/user'].avatar"
          :user-id="$store.getters['auth/user'].id"
        />
      </AccountMenu>
    </div>
    <div v-else>
      <v-btn text to="/login" nuxt>
        login
      </v-btn>
    </div>
  </v-app-bar>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'
import AccountMenu from '@/components/account/AccountMenu.vue'
import CalendarController from '@/components/calendar/Controller.vue'

@Component({
  components: {
    AccountMenu,
    CalendarController
  }
})
class AppHeader extends Vue {
}
export default AppHeader
</script>

<style lang="scss" scoped>
@font-face {
  font-family: Logo;
  src: url(/UniSansHeavy.otf);
}

#logo {
  font-size: 1.2em;
  font-family: Logo, sans-serif;
}
</style>
