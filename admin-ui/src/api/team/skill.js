import request from '@/utils/request'

// 查询团队技能列表
export function listTeamSkill(query) {
  return request({
    url: '/api/team/skills/list',
    method: 'get',
    params: query
  })
}

// 查询团队技能详细
export function getTeamSkill(id) {
  return request({
    url: '/api/team/skills/' + id,
    method: 'get'
  })
}

// 新增团队技能
export function addTeamSkill(data) {
  return request({
    url: '/api/team/skills',
    method: 'post',
    data: data
  })
}

// 修改团队技能
export function updateTeamSkill(data) {
  return request({
    url: '/api/team/skills',
    method: 'put',
    data: data
  })
}

// 删除团队技能
export function delTeamSkill(ids) {
  return request({
    url: '/api/team/skills/' + ids,
    method: 'delete'
  })
}
