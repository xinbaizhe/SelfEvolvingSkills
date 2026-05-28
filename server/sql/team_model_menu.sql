-- 部门模型管理菜单：挂到“系统管理”下。
-- 业务数据复用 team_model_configs，不需要新建模型配置表。
-- 对已有数据库执行本文件后，重新登录 admin-ui 即可在“系统管理”下看到菜单。

set @system_menu_id := (
  select menu_id
  from sys_menu
  where parent_id = 0 and path = 'system'
  order by menu_id
  limit 1
);

insert into sys_menu (
  menu_name, parent_id, order_num, path, component, `query`, route_name,
  is_frame, is_cache, menu_type, visible, status, perms, icon,
  create_by, create_time, update_by, update_time, remark
)
select '部门模型管理', @system_menu_id, 10, 'model', 'team/model/index', '', '',
       1, 0, 'C', '0', '0', 'system:model:list', 'server',
       'admin', sysdate(), '', null, '部门模型管理菜单'
where @system_menu_id is not null
  and not exists (
    select 1 from sys_menu where parent_id = @system_menu_id and path = 'model'
  );

update sys_menu
set menu_name = '部门模型管理',
    component = 'team/model/index',
    perms = 'system:model:list',
    icon = 'server',
    visible = '0',
    status = '0',
    menu_type = 'C'
where parent_id = @system_menu_id
  and path = 'model';

set @model_menu_id := (
  select menu_id
  from sys_menu
  where parent_id = @system_menu_id and path = 'model'
  order by menu_id
  limit 1
);

insert into sys_menu (
  menu_name, parent_id, order_num, path, component, `query`, route_name,
  is_frame, is_cache, menu_type, visible, status, perms, icon,
  create_by, create_time, update_by, update_time, remark
)
select '部门模型查询', @model_menu_id, 1, '#', '', '', '',
       1, 0, 'F', '0', '0', 'system:model:query', '#',
       'admin', sysdate(), '', null, ''
where @model_menu_id is not null
  and not exists (select 1 from sys_menu where perms = 'system:model:query');

insert into sys_menu (
  menu_name, parent_id, order_num, path, component, `query`, route_name,
  is_frame, is_cache, menu_type, visible, status, perms, icon,
  create_by, create_time, update_by, update_time, remark
)
select '部门模型新增', @model_menu_id, 2, '#', '', '', '',
       1, 0, 'F', '0', '0', 'system:model:add', '#',
       'admin', sysdate(), '', null, ''
where @model_menu_id is not null
  and not exists (select 1 from sys_menu where perms = 'system:model:add');

insert into sys_menu (
  menu_name, parent_id, order_num, path, component, `query`, route_name,
  is_frame, is_cache, menu_type, visible, status, perms, icon,
  create_by, create_time, update_by, update_time, remark
)
select '部门模型修改', @model_menu_id, 3, '#', '', '', '',
       1, 0, 'F', '0', '0', 'system:model:edit', '#',
       'admin', sysdate(), '', null, ''
where @model_menu_id is not null
  and not exists (select 1 from sys_menu where perms = 'system:model:edit');

insert into sys_menu (
  menu_name, parent_id, order_num, path, component, `query`, route_name,
  is_frame, is_cache, menu_type, visible, status, perms, icon,
  create_by, create_time, update_by, update_time, remark
)
select '部门模型删除', @model_menu_id, 4, '#', '', '', '',
       1, 0, 'F', '0', '0', 'system:model:remove', '#',
       'admin', sysdate(), '', null, ''
where @model_menu_id is not null
  and not exists (select 1 from sys_menu where perms = 'system:model:remove');
