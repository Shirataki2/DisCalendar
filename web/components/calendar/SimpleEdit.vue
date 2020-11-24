<template>
  <v-card flat min-width="350px">
    <v-toolbar v-if="event" dark :color="event.color">
      <v-toolbar-title v-text="event.name" />
    </v-toolbar>
  </v-card>
</template>

<script lang="ts">
import { Component, Vue, Prop } from 'nuxt-property-decorator'

@Component
class Edit extends Vue {
  @Prop({ required: true })
  event!: any
}
export default Edit
</script>
