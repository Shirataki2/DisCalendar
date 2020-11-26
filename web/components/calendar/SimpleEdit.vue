<template>
  <v-card flat min-width="350px">
    <v-toolbar v-if="event" flat dark :color="event.color">
      <v-toolbar-title v-text="event.name" />
    </v-toolbar>
    <v-container>
      <v-row>
        <v-col cols="12">
          <v-icon class="mt-n1 mr-1">
            mdi-alarm
          </v-icon>
          <span style="word-wrap: break-word" v-text="date" /></span>
        </v-col>
      </v-row>
    </v-container>
  </v-card>
</template>

<script lang="ts">
import { Component, Vue, Prop } from 'nuxt-property-decorator'

@Component
class Edit extends Vue {
  @Prop({ required: true })
  event!: any

  get date () {
    const start = this.$moment(this.event.start)
    const end = this.$moment(this.event.end)
    if (this.event.is_all_day) {
      if (start.format('YYYY-MM-DD') !== end.format('YYYY-MM-DD')) {
        return `${start.format('YYYY/MM/DD')} - ${end.format('YYYY/MM/DD')}`
      } else {
        return `${start.format('YYYY/MM/DD')}`
      }
    } else if (start.format('YYYY-MM-DD') !== end.format('YYYY-MM-DD')) {
      return `${start.format('YYYY/MM/DD HH:mm')} - ${end.format('YYYY/MM/DD HH:mm')}`
    } else {
      return `${start.format('YYYY/MM/DD HH:mm')} - ${end.format('HH:mm')}`
    }
  }
}
export default Edit
</script>
