<template>
  <v-row justify="center" align="center">
    <v-col v-if="server" class="my-n3" cols="12">
      <v-btn large color="secondary" rounded @click.stop="eventDialog = true">
        <v-icon>
          mdi-plus
        </v-icon>
        新規作成
      </v-btn>
      <v-dialog v-model="eventDialog" max-width="1000" min-width="80%" persistent>
        <NewEvent
          :endpoint="`/local/api/events/${$route.params.id}`"
          method="POST"
          @submitted="onSubmitted"
          @cancel="eventDialog = false"
        />
      </v-dialog>
    </v-col>
    <v-col v-if="!server" class="my-n3 text-right" cols="6">
      <v-btn large icon>
        <v-icon>
          mdi-cog
        </v-icon>
      </v-btn>
    </v-col>
    <v-col v-if="server" class="mt-2 mb-n5 mx-n3" cols="12">
      <div style="width: 100%">
        <Calendar ref="calendar" @submitted="onSubmitted" />
      </div>
    </v-col>
    <v-col v-else style="text-align: center" cols="12">
      <h1 class="my-5">
        サーバーデータの取得に失敗しました
      </h1>
      <p>以下の事項をご確認ください</p>
      <p>
        ･ BOTがサーバーに導入されているか
      </p>
      <p>
        ･ あなた自身がBOTを導入したサーバーに参加しているか
      </p>
    </v-col>
  </v-row>
</template>

<script lang="ts">
import { Component, Vue, Prop } from 'nuxt-property-decorator'
import Calendar from '@/components/calendar/Calendar.vue'
import NewEvent from '@/components/calendar/NewEvent.vue'

@Component({
  layout: 'authorized',
  components: { Calendar, NewEvent },
  asyncData: async ({ redirect, store, app, $axios }) => {
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
        redirect(301, '/login')
      }
    }
  }
})
class Index extends Vue {
  @Prop({ type: Object })
  prev!: any

  @Prop({ type: Object })
  next!: any

  eventDialog: boolean = false

  get server () {
    const server = this.$store.getters['auth/server']
    return server
  }

  async onSubmitted () {
    const cal: any = this.$refs.calendar
    await cal.updateCalendar()
    this.eventDialog = false
  }
}
export default Index
</script>
