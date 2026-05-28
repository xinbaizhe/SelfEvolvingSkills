import request from '@/utils/request'

export function listTeamModel(query) {
  return request({
    url: '/api/team/models/list',
    method: 'get',
    params: query
  })
}

export function getTeamModel(id) {
  return request({
    url: '/api/team/models/' + id,
    method: 'get'
  })
}

export function addTeamModel(data) {
  return request({
    url: '/api/team/models',
    method: 'post',
    data: data
  })
}

export function updateTeamModel(data) {
  return request({
    url: '/api/team/models',
    method: 'put',
    data: data
  })
}

export function delTeamModel(ids) {
  return request({
    url: '/api/team/models/' + ids,
    method: 'delete'
  })
}
