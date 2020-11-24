<template>
  <v-sheet :height="height">
    <v-calendar
      id="calendar"
      ref="calendar"
      v-model="date"
      color="primary"
      :type="type"
      :events="events"
      :event-color="getEventColor"
      :event-overlap-threshold="30"
      :event-name="eventName"
      @change="getEvents"
      @contextmenu:event="editEvent"
      @mousedown:event="clickEvent"
      @mousedown:time="clickTime"
      @mousemove:time="moveEvent"
      @mouseup:time="endEvent"
      @mouseleave.native="cancelDrag"
    >
      <template #day-body="{ date, week }">
        <div
          class="v-current-time"
          :class="{ first: date === week[0].date }"
          :style="{ top: offsetY }"
        />
      </template>
      <template #event="{ eventParsed, timed, event }">
        <div
          class="v-event-draggable"
        >
          <div v-if="type === 'month'">
            <strong v-text="eventDate(eventParsed)" />
            <span v-text="eventName(eventParsed)" />
          </div>
          <div v-else class="mx-1">
            <span v-text="eventName(eventParsed)" />
            <br>
            <strong v-text="eventDate(eventParsed)" />
          </div>
        </div>
        <div
          v-if="timed"
          class="v-event-drag-bottom"
          @mousedown.stop="extendBottom(event)"
        />
      </template>
    </v-calendar>
    <v-menu
      v-model="editDialog"
      :close-on-content-click="false"
      :activator="selectedElement"
      offset-x
    >
      <Edit :event="event" />
    </v-menu>
  </v-sheet>
</template>

<script lang="ts">
import { Component, Vue } from 'nuxt-property-decorator'
import Edit from '@/components/calendar/SimpleEdit.vue'
type dateTypes = 'month' | 'week' | 'day' | '4day'

@Component({
  components: { Edit }
})
class Calendar extends Vue {
  date = this.$moment().format('YYYY-MM-DD')
  ready = false
  ref: any = null
  type: dateTypes = 'month'
  events: any[] = []
  colors = ['#2196F3', '#3F51B5', '#673AB7', '#00BCD4', '#4CAF50', '#FF9800', '#757575']
  names = ['Meeting', 'Holiday', 'PTO', 'Travel', 'Event', 'Birthday', 'Conference', 'Party']
  height = '600px'
  offsetY = '-10px'
  editDialog = false
  event: any = null
  selectedElement: any = null

  calcOffset () {
    if (this.ref) {
      const offY = 48 * this.$moment().hour() +
                   48 * this.$moment().minute() / 60
      return ~~offY + 'px'
    } else {
      return '-10px'
    }
  }

  async mounted () {
    const refs: any = this.$refs
    this.ref = refs.calendar
    this.setHeight()
    window.addEventListener('resize', this.setHeight)
    this.updateDate()
    this.offsetY = this.calcOffset()
    setInterval(() => {
      this.offsetY = this.calcOffset()
    }, 10000)
    this.$store.subscribe((mut, _) => {
      if (mut.type === 'calendar/TODAY') {
        this.today()
      }
      if (mut.type === 'calendar/PREV' && mut.payload) {
        this.prev()
      }
      if (mut.type === 'calendar/NEXT' && mut.payload) {
        this.next()
      }
      if (mut.type === 'calendar/TYPE') {
        this.setType(mut.payload)
      }
      if (mut.type === 'calendar/DATE') {
        const d = mut.payload
        this.date = `${d[0]}-${d[1]}-${d[2]}`
      }
    })
    await this.$nextTick()
    this.scrollToNow()
  }

  scrollToNow () {
    const now = this.$moment().hour() * 60 + this.$moment().minute()
    this.ref.scrollToTime(Math.max(0, now - (now % 30) - 30))
  }

  updateDate () {
    this.$store.commit('calendar/DATE', [
      this.$moment(this.date).year(),
      this.$moment(this.date).month() + 1,
      this.$moment(this.date).date()
    ])
  }

  eventDate ({ start, end, allDay }: any) {
    if (allDay || !start.hasTime) {
      return ''
    } else if (end.hasTime && this.type !== 'month') {
      return `${start.time}-${end.time}`
    } else {
      return `${start.time}`
    }
  }

  eventName ({ input }: any) {
    return input.name
  }

  setHeight () {
    const c = document.getElementById('calendar')
    const y = 1.5 * (c?.getBoundingClientRect().y || 0)
    this.height = `${window.innerHeight - ~~y}px`
  }

  today () {
    this.date = this.$moment().format('YYYY-MM-DD')
    this.updateDate()
  }

  prev () {
    this.ref.prev()
    this.updateDate()
  }

  next () {
    this.ref.next()
    this.updateDate()
  }

  setType (type: dateTypes) {
    this.scrollToNow()
    this.type = type
  }

  dragEvent: any = null
  dragTime: any = null
  extend: any = null
  extendEvent: any = null
  extendStart: any = null

  clickEvent ({ event, timed }: any) {
    if (event && timed) {
      this.dragEvent = event
      this.dragTime = null
      this.extend = null
    }
  }

  clickTime (tms: any) {
    const mouse = this.toTime(tms)
    if (this.dragEvent && this.dragTime === null) {
      const start = this.dragEvent.start
      this.dragTime = mouse - start
    }
  }

  extendBottom (e: any) {
    this.extendEvent = e
    this.extendStart = e.start
    this.extend = e.end
  }

  toTime (tms: any) {
    return new Date(tms.year, tms.month - 1, tms.day, tms.hour, tms.minute).getTime()
  }

  roundTime (time: any, down = true) {
    const roundTo = 15 // minutes
    const roundDownTime = roundTo * 60 * 1000

    return down
      ? time - time % roundDownTime
      : time + (roundDownTime - (time % roundDownTime))
  }

  moveEvent (tms: any) {
    const mouse = this.toTime(tms)
    if (this.dragEvent && this.dragTime !== null) {
      const start = this.dragEvent.start
      const end = this.dragEvent.end
      const duration = end - start
      const newStartTime = mouse - this.dragTime
      const newStart = this.roundTime(newStartTime)
      const newEnd = newStart + duration

      this.dragEvent.start = newStart
      this.dragEvent.end = newEnd
    } else if (this.extendEvent && this.extendStart !== null) {
      const mouseRounded = this.roundTime(mouse, false)
      const min = Math.min(mouseRounded, this.extendStart)
      const max = Math.max(mouseRounded, this.extendStart)

      this.extendEvent.start = min
      this.extendEvent.end = max
    }
  }

  endEvent () {
    this.dragTime = null
    this.dragEvent = null
    this.extendEvent = null
    this.extendStart = null
    this.extend = null
  }

  cancelDrag () {
    if (this.extendEvent) {
      if (this.extend) {
        this.extendEvent.end = this.extend
      } else {
        const i = this.events.indexOf(this.extendEvent)
        if (i !== -1) {
          this.events.splice(i, 1)
        }
      }
    }

    this.extendEvent = null
    this.extendStart = null
    this.dragTime = null
    this.dragEvent = null
  }

  editEvent ({ event, nativeEvent }: any) {
    const open = () => {
      this.event = event
      this.selectedElement = nativeEvent.target
      setTimeout(() => { this.editDialog = true }, 10)
    }
    if (this.editDialog) {
      this.editDialog = false
      setTimeout(open, 10)
    } else {
      open()
    }
  }

  getEventColor (event: any) {
    return event.color
  }

  getEvents ({ start, end }: any) {
    const events = []

    const min = new Date(`${start.date}T00:00:00`).getTime()
    const max = new Date(`${end.date}T23:59:59`).getTime()
    const days = (max - min) / 86400000
    const eventCount = this.rnd(days, days + 20)

    for (let i = 0; i < eventCount; i++) {
      const timed = this.rnd(0, 3) !== 0
      const firstTimestamp = this.rnd(min, max)
      const secondTimestamp = this.rnd(2, timed ? 8 : 288) * 900000
      const start = firstTimestamp - (firstTimestamp % 900000)
      const end = start + secondTimestamp

      events.push({
        name: this.rndElement(this.names),
        color: this.rndElement(this.colors),
        start,
        end,
        timed
      })
    }

    this.events = events
  }

  rnd (a: number, b: number) {
    return Math.floor((b - a + 1) * Math.random()) + a
  }

  rndElement (arr: Array<any>) {
    return arr[this.rnd(0, arr.length - 1)]
  }
}
export default Calendar
</script>

<style lang="scss">
.v-current-time {
  height: 2px;
  background-color: #ea4335;
  position: absolute;
  left: -1px;
  right: 0;
  pointer-events: none;

  &.first::before {
    content: '';
    position: absolute;
    background-color: #ea4335;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    margin-top: -5px;
    margin-left: -6.5px;
  }
}

.v-event-draggable {
  padding-left: 6px;
}

.v-event-timed {
  user-select: none;
  -webkit-user-select: none;
}

.v-event-drag-bottom {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 5px;
  height: 5px;
  cursor: ns-resize;

  &::after {
    display: none;
    position: absolute;
    left: 50%;
    height: 4px;
    border-top: 1px solid white;
    border-bottom: 1px solid white;
    width: 16px;
    margin-left: -8px;
    opacity: 0.8;
    content: '';
  }

  &:hover::after {
    display: block;
  }
}
</style>
