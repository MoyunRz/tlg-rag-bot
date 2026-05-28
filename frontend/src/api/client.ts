import axios from 'axios'

const client = axios.create({
  baseURL: '/api',
  timeout: 180000,
})

export default client
