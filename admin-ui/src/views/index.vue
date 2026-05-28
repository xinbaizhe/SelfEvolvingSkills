<template>
  <div class="app-container home">
    <el-row :gutter="20">
      <el-col :sm="24" :lg="12" style="padding-left: 20px">
        <h2>Self Evolving Skills</h2>
        <p>
          本地优先的工作流助手，从你的重复性任务中学习。扫描、索引并从日常工作中进化出可复用的技能（Skills），所有数据均在本地处理。通过团队版服务器与团队共享和同步技能。
        </p>
        <p>
          <b>当前版本：</b> <span>v{{ version }}</span>
        </p>
        <p>
          <el-tag type="success">本地隐私</el-tag>
          <el-tag type="warning" style="margin-left: 8px">团队同步</el-tag>
        </p>
      </el-col>

      <el-col :sm="24" :lg="12" style="padding-left: 50px">
        <el-row>
          <el-col :span="24">
            <h2>技术栈</h2>
          </el-col>
        </el-row>
        <el-row>
          <el-col :span="8">
            <h4>桌面端</h4>
            <ul>
              <li>Tauri</li>
              <li>Vue 3</li>
              <li>TypeScript</li>
              <li>Rust</li>
            </ul>
          </el-col>
          <el-col :span="8">
            <h4>管理端 / 服务端</h4>
            <ul>
              <li>Spring Boot 3</li>
              <li>Spring Security</li>
              <li>JWT</li>
              <li>MyBatis</li>
            </ul>
          </el-col>
          <el-col :span="8">
            <h4>管理界面</h4>
            <ul>
              <li>Vue 3</li>
              <li>Element Plus</li>
              <li>Pinia</li>
              <li>Vite</li>
            </ul>
          </el-col>
        </el-row>
      </el-col>
    </el-row>
    <el-divider />
    <el-row :gutter="20">
      <el-col :xs="24" :sm="24" :md="12" :lg="8">
        <el-card class="update-log">
          <template v-slot:header>
            <div class="clearfix">
              <span>功能特性</span>
            </div>
          </template>
          <div class="body">
            <ul>
              <li>扫描本地文件，检测可重复的工作流模式</li>
              <li>从检测到的模式中自动生成技能（Skills）</li>
              <li>在本地库中管理和组织技能</li>
              <li>团队版：跨组织共享技能</li>
              <li>协同技能进化与审核工作流</li>
              <li>集中化的模型配置管理</li>
            </ul>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="24" :md="12" :lg="8">
        <el-card class="update-log">
          <template v-slot:header>
            <div class="clearfix">
              <span>快速开始</span>
            </div>
          </template>
          <div class="body">
            <ol>
              <li>运行桌面应用，扫描你的工作目录</li>
              <li>查看生成的技能建议</li>
              <li>将技能安装到本地智能体工作区</li>
              <li>启动团队服务器进行团队协作</li>
              <li>与团队成员共享和发现技能</li>
            </ol>
          </div>
        </el-card>
      </el-col>
      <el-col :xs="24" :sm="24" :md="12" :lg="8">
        <el-card class="update-log">
          <template v-slot:header>
            <div class="clearfix">
              <span>相关链接</span>
            </div>
          </template>
          <div class="body">
            <p>
              <el-icon><Link /></el-icon>
              <el-link href="https://github.com/xinbaizhe/SelfEvolvingSkills" target="_blank">GitHub 仓库</el-link>
            </p>
          </div>
        </el-card>
      </el-col>
    </el-row>
  </div>
</template>

<script setup name="Index">
const version = ref('2.0.1')

function goTarget(url) {
  window.open(url, '__blank')
}
</script>

<style scoped lang="scss">
.home {
  blockquote {
    padding: 10px 20px;
    margin: 0 0 20px;
    font-size: 17.5px;
    border-left: 5px solid #eee;
  }
  hr {
    margin-top: 20px;
    margin-bottom: 20px;
    border: 0;
    border-top: 1px solid #eee;
  }
  .col-item {
    margin-bottom: 20px;
  }

  ul {
    padding: 0;
    margin: 0;
  }

  font-family: "open sans", "Helvetica Neue", Helvetica, Arial, sans-serif;
  font-size: 13px;
  color: #676a6c;
  overflow-x: hidden;

  ul {
    list-style-type: none;
    line-height: 1.8;
  }

  ol {
    line-height: 1.8;
  }

  h4 {
    margin-top: 0px;
  }

  h2 {
    margin-top: 10px;
    font-size: 26px;
    font-weight: 100;
  }

  p {
    margin-top: 10px;
    line-height: 1.6;

    b {
      font-weight: 700;
    }
  }

  .update-log {
    min-height: 220px;
  }
}
</style>
