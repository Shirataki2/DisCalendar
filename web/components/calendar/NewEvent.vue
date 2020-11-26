<template>
  <v-card flat>
    <v-card-title class="text-h5 font-weight-bold" v-text="cardTitle" />
    <v-card-text>
      <v-form ref="form">
        <v-container>
          <v-row>
            <v-col cols="12">
              <v-text-field
                v-model="title"
                label="タイトルを入力 *"
                :rules="[
                  v => !!v || 'この項目は必須です',
                  v => v.length <= 32 || 'この項目は32文字以下である必要があります'
                ]"
                counter="32"
              />
            </v-col>
            <v-col class="mt-n6" cols="12" sm="5">
              <v-menu
                v-model="startDateMenu"
                :close-on-content-click="false"
                transition="scale-transition"
                offset-y
                min-width="250px"
              >
                <template #activator="{ on, attrs }">
                  <v-combobox
                    v-model="startDate"
                    label="開始日"
                    prepend-icon="mdi-calendar"
                    readonly
                    v-bind="attrs"
                    v-on="on"
                  />
                </template>
                <v-date-picker
                  v-model="startDate"
                  no-title
                  scrollable
                >
                  <v-spacer />
                  <v-btn text color="primary" @click="startDateMenu = false">
                    OK
                  </v-btn>
                </v-date-picker>
              </v-menu>
            </v-col>
            <v-col class="mt-n6" cols="12" sm="5">
              <v-menu
                v-model="startTimeMenu"
                :disabled="isAllDay"
                :close-on-content-click="false"
                transition="scale-transition"
                offset-y
                min-width="250px"
              >
                <template #activator="{ on, attrs }">
                  <v-combobox
                    v-model="startTime"
                    :disabled="isAllDay"
                    label="開始時"
                    prepend-icon="mdi-clock-time-eight-outline"
                    readonly
                    v-bind="attrs"
                    v-on="on"
                  />
                </template>
                <v-time-picker
                  v-model="startTime"
                  :landscape="$vuetify.breakpoint.smAndUp"
                  scrollable
                >
                  <v-spacer />
                  <v-btn text color="primary" @click="startTimeMenu = false">
                    OK
                  </v-btn>
                </v-time-picker>
              </v-menu>
            </v-col>
            <v-col class="mt-n6" cols="12" sm="2">
              <v-checkbox
                v-model="isAllDay"
                label="終日"
              />
            </v-col>
            <v-col class="mt-n9" cols="12" sm="5">
              <v-menu
                v-model="endDateMenu"
                :close-on-content-click="false"
                transition="scale-transition"
                offset-y
                min-width="250px"
              >
                <template #activator="{ on, attrs }">
                  <v-combobox
                    v-model="endDate"
                    label="終了日"
                    prepend-icon="mdi-calendar"
                    readonly
                    v-bind="attrs"
                    v-on="on"
                  />
                </template>
                <v-date-picker
                  v-model="endDate"
                  no-title
                  scrollable
                >
                  <v-spacer />
                  <v-btn text color="primary" @click="endDateMenu = false">
                    OK
                  </v-btn>
                </v-date-picker>
              </v-menu>
            </v-col>
            <v-col class="mt-n9" cols="12" sm="5">
              <v-menu
                v-model="endTimeMenu"
                :disabled="isAllDay"
                :close-on-content-click="false"
                transition="scale-transition"
                offset-y
                min-width="250px"
              >
                <template #activator="{ on, attrs }">
                  <v-combobox
                    v-model="endTime"
                    :disabled="isAllDay"
                    label="終了時"
                    prepend-icon="mdi-clock-time-eight-outline"
                    readonly
                    v-bind="attrs"
                    v-on="on"
                  />
                </template>
                <v-time-picker
                  v-model="endTime"
                  :landscape="$vuetify.breakpoint.smAndUp"
                  scrollable
                >
                  <v-spacer />
                  <v-btn text color="primary" @click="endTimeMenu = false">
                    OK
                  </v-btn>
                </v-time-picker>
              </v-menu>
            </v-col>
            <v-col class="mt-n4" cols="12">
              <v-icon>mdi-bell</v-icon>
              <NotificationBtn
                v-for="notification in notifications"
                :key="notification.key"
                class="mx-1"
                :num.sync="notification.num"
                :selected.sync="notification.type"
                @remove="deleteNotification(notification)"
              />
              <v-btn icon @click="addNotification">
                <v-icon>
                  mdi-plus
                </v-icon>
              </v-btn>
            </v-col>
            <v-col class="mt-n6" cols="12">
              <v-textarea
                v-model="description"
                label="説明"
                :rules="[
                  v => v.length <= 500 || 'この項目は500文字以下である必要があります'
                ]"
                counter="500"
              />
            </v-col>
          </v-row>
          <v-btn block outlined color="primary" class="mt-4" @click="submit">
            OK
          </v-btn>
          <v-btn block outlined color="error" class="mt-4" @click="cancel">
            キャンセル
          </v-btn>
          <v-btn
            v-if="deletable"
            block
            outlined
            color="error"
            class="mt-4"
            @click="deleteEvent"
          >
            削除
          </v-btn>
        </v-container>
      </v-form>
    </v-card-text>
  </v-card>
</template>

<script lang="ts">
import { Component, Vue, Prop } from 'nuxt-property-decorator'
import NotificationBtn from '@/components/buttons/NotificationBtn.vue'

interface Notification {
  key: number
  num: number
  type: string
}

@Component({
  components: { NotificationBtn }
})
class New extends Vue {
  @Prop({ type: Boolean, default: false })
  deletable!: boolean

  @Prop({ type: String, default: '新規作成' })
  cardTitle!: string

  title: string = ''
  description: string = ''
  startDateMenu: boolean = false
  startDate: string = this.$moment().format('YYYY-MM-DD')
  startTimeMenu: boolean = false
  startTime: string = this.$moment().format('HH:00')
  isAllDay: boolean = false
  endDateMenu: boolean = false
  endDate: string = this.$moment().format('YYYY-MM-DD')
  endTimeMenu: boolean = false
  endTime: string = this.$moment().format('HH:30')

  notifications: Array<Notification> = [
    { key: 0, num: 1, type: '日前' },
    { key: 1, num: 1, type: '時間前' }
  ]

  key = 2

  get form () {
    const ref: any = this.$refs.form
    return ref
  }

  addNotification () {
    const key = this.key
    this.key++
    this.notifications.push({ key, num: key, type: '時間前' })
  }

  deleteNotification (target: Notification) {
    this.notifications = this.notifications.filter(
      notification => notification.key !== target.key
    )
  }

  submit () {}

  cancel () {
    this.$emit('cancel')
  }

  deleteEvent () {}
}
export default New
</script>
