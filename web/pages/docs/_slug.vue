<template>
  <div>
    <v-container>
      <v-row>
        <v-col cols="12" lg="10" xl="8" offset-lg="1" offset-xl="2">
          <h1 class="text-h4">
            {{ page.title }}
          </h1>
        </v-col>
      </v-row>
      <v-divider />
      <v-row>
        <v-col cols="12" lg="10" xl="8" offset-lg="1" offset-xl="2">
          <nuxt-content id="docs" :document="page" />
        </v-col>
      </v-row>
    </v-container>
  </div>
</template>

<script lang="ts">
import { Vue, Component } from 'vue-property-decorator'
import { Page } from '@/types'

@Component({
  name: 'Support',
  asyncData: async ({ $content, params, error }) => {
    const slug = params.slug
    if (!slug) { error({ statusCode: 404, message: 'Page not Found' }) }
    const page = await $content('docs', slug).fetch()
    return { page }
  }
})
class Support extends Vue {
  page!: Page
}
export default Support
</script>

<style lang="scss">
#docs {
  h2 {
    margin-top: 20px !important;
    margin-bottom: 10px !important;
  }

  li {
    margin: 6px 1px;
  }

  pre::before {
    content: '';
  }

  pre {
    background: #444 !important;
    code {
      margin: 0 -2px;
    }
  }

  code {
    font-family: 'Courier Prime', monospace !important;
    background: #444 !important;
    color: #fff !important;
    padding: 4px 2px !important;
    margin: 0 4px;
    font-weight: 300 !important;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    border-spacing: 0;
  }

  table th,
  table td {
    padding: 10px 0;
    text-align: center;
  }

  table tr:nth-child(even) {
    background-color: #333;
    color: #ccc;
  }
}
</style>
