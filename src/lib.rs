use anyhow::{anyhow, Result};
use chrono::{FixedOffset, Timelike, Utc};

use jd_com::{account::JAccount, sign::get_sign};
use log::info;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

// 定义错误类型
#[derive(Error, Debug)]
enum JError {
    #[error("请求数据失败")]
    RequestFailure,

    #[error("解析数据失败")]
    ParseFailure,
}

// 果树信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct JdFarmInfo {
    // 当前剩余的总水滴
    total_energy: u32,

    // 果树状态
    tree_state: u8,

    // 当前树已浇水滴
    tree_energy: u32,

    // 果树升级/成熟需要的水滴
    tree_total_energy: u32,

    // 助力码
    share_code: String,

    // 用户昵称
    nick_name: String,

    // 奖品名称
    name: String,

    // 奖品等级
    prize_level: u8,
}

// 签到任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SignInTask {
    // 是否已完成
    f: bool,
}

// 首次浇水任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FirstWaterTask {
    // 是否已完成
    f: bool,
}

// 十次浇水任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TotalWaterTask {
    // 是否已完成
    f: bool,
    // 总共需要浇水次数
    total_water_task_limit: u16,
    // 当前已浇水次数
    total_water_task_times: u16,
}

// 给好友浇水任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WaterFriendTask {
    // 总共需要为好友浇水的次数
    water_friend_max: u8,

    // 当前为好友浇水的次数
    water_friend_count_key: u8,

    // 是否已完成
    f: bool,

    // 奖励是否已领取
    water_friend_got_award: bool,
}

// 浏览任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BrowseTaskItem {
    // 广告ID
    advert_id: String,
    // 任务名称
    main_title: String,
    // 最多完成次数
    limit: u8,
    // 已完成次数
    had_finished_times: u8,
    // 任务等待时间
    time: u16,
    // 领取奖励的次数
    had_got_times: u8,
}

// 浏览类型任务列表
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BrowseTask {
    // 是否完成
    f: bool,
    // 子任务列表
    user_browse_task_ads: Vec<BrowseTaskItem>,
}

// 从App首页进入农场
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TreasureBoxTask {
    line: String,
    f: bool,
}

// 水滴雨任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WaterRainTask {
    f: bool,
    win_times: u8,
    last_time: u64,
}

// 好友信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FriendInfo {
    // 好友昵称
    nick_name: String,
    // 好友助力码
    share_code: String,
    // 是否可以帮他浇水
    friend_state: u8,
}

// 好友列表
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FriendInfoList {
    // 好友信息列表
    friends: Vec<FriendInfo>,
}

// 三餐定时领水
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ThreeMealTask {
    // 是否已完成
    f: bool,
}

// 任务信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct TaskInfo {
    // 签到任务
    sign_init: SignInTask,
    // 首次浇水任务
    first_water_init: FirstWaterTask,
    // 十次浇水任务
    total_water_task_init: TotalWaterTask,
    // 为两位好友浇水任务
    water_friend_task_init: WaterFriendTask,
    // 浏览商品任务
    got_browse_task_ad_init: BrowseTask,
    // 从首页免费水果进入农场
    treasure_box_init: TreasureBoxTask,
    // 水滴雨任务
    water_rain_init: WaterRainTask,
    // 三餐定时领水任务
    got_three_meal_init: ThreeMealTask,
}

// 签到领水->关注任务
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FollowTask {
    // 广告ID
    advert_id: String,
    // 任务ID
    id: String,
    // 任务名称
    name: String,
    // 是否领取奖励
    had_got: bool,
    // 是否已关注
    had_follow: bool,
}

// 签到领水任务信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ClockInTask {
    // 是否已签到
    today_signed: bool,
    // 限时关注领水滴任务列表
    themes: Vec<FollowTask>,
}

// 背包道具卡信息
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct CardInfo {
    // 水滴翻倍卡
    double_card: u16,
    // 快速浇水卡
    fast_card: u16,
    // 加签卡
    sign_card: u16,
    // 水滴换豆卡
    bean_card: u16,
}

pub struct JClient {
    client: Client,
    base_url: String,
    account: JAccount,
}

impl JClient {
    pub fn new(account: JAccount) -> Self {
        let mut headers = HeaderMap::new();

        headers.append(
            "cookie",
            HeaderValue::from_str(account.cookie().as_str()).unwrap(),
        );
        headers.append(
            "referer",
            HeaderValue::from_str("https://carry.m.jd.com/").unwrap(),
        );

        headers.append(
            "referer",
            HeaderValue::from_str("https://carry.m.jd.com").unwrap(),
        );

        let client = Client::builder()
            .default_headers(headers)
            .user_agent("JD4iPhone/168328 (iPhone; iOS; Scale/3.00)")
            .build()
            .unwrap();
        let base_url = "https://api.m.jd.com/client.action".to_string();
        Self {
            client,
            base_url,
            account,
        }
    }

    // 请求数据
    // function_id: &str
    // body: &string
    async fn request(&self, function_id: &str, body: &str) -> Result<Value> {
        let sign = get_sign(function_id, body);
        let url = format!("{}?{}&appid=signed_wh5", self.base_url, sign);
        let res = self
            .client
            .post(url)
            .body(format!("body={:?}", body))
            .send()
            .await?
            .json::<Value>()
            .await
            .map_err(|_| JError::RequestFailure);

        match res {
            Ok(data) => match data.get("code").is_some() {
                true => Ok(data),
                false => Ok(json!({"code": "888"})),
            },
            Err(e) => Ok(json!({"code": "999", "message": e.to_string()})),
        }
    }

    // 获取农场数据
    async fn get_farm_data(&self) -> Result<Value> {
        // toBeginEnergy: 发芽需要的水滴
        // toFlowEnergy:  开花状态需要的水滴
        // toFruitTimes:  结果状态需要的浇水次数
        let res = self
            .request(
                "initForFarm",
                r#"{"babelChannel":"121","sid":"","un_area":"","version":18,"channel":1}"#,
            )
            .await
            .map_err(|_| JError::RequestFailure)?;
        Ok(res)
    }

    async fn get_farm_info(&self, farm_data: Option<Value>) -> Result<JdFarmInfo> {
        let farm_data = match farm_data {
            Some(data) => data,
            None => self.get_farm_data().await?,
        };
        Ok(serde_json::from_value(farm_data["farmUserPro"].clone())
            .map_err(|_| JError::ParseFailure)?)
    }

    // 是否操作成功
    fn is_success(&self, data: &Value) -> bool {
        data["code"].as_str().unwrap_or("999") == "0"
    }

    // 完成弹出的领水任务
    async fn do_pop_task(&self) -> Result<()> {
        let res = self
            .request(
                "gotWaterGoalTaskForFarm",
                r#"{"type":3,"version":18,"channel":1,"babelChannel":"121"}"#,
            )
            .await?;

        if self.is_success(&res) {
            let energy = res["addEnergy"].as_u64().unwrap_or(0);
            info!(
                "{}, 成功完成弹出任务, 获得水滴:{}g!",
                self.account.name(),
                energy
            );
        } else {
            info!("{}, 无法完成弹出任务, {}", self.account.name(), res);
        }
        Ok(())
    }

    // 获取任务信息
    async fn get_task_info(&self) -> Result<TaskInfo> {
        let res = self
            .request(
                "taskInitForFarm",
                r#"{"version":18,"channel":1,"babelChannel":"121"}"#,
            )
            .await
            .map_err(|_| JError::RequestFailure)?;

        match self.is_success(&res) {
            true => Ok(serde_json::from_value(res)?),
            false => Err(anyhow!(JError::RequestFailure)),
        }
    }

    // 浇水一次
    async fn water(&self) -> Result<bool> {
        let res = self
            .request(
                "waterGoodForFarm",
                r#"{"type":"","version":18,"channel":1,"babelChannel":"121"}"#,
            )
            .await
            .map_err(|_| JError::RequestFailure)?;

        Ok(match self.is_success(&res) {
            true => {
                let total_energy = res["totalEnergy"].as_u64().unwrap_or(0);
                info!(
                    "{}, 成功浇水一次, 剩余水滴:{}g!",
                    self.account.name(),
                    total_energy
                );
                true
            }
            false => {
                info!("{}, 浇水失败, {}", self.account.name(), res);
                false
            }
        })
    }

    // 签到任务
    async fn sign_in(&self) -> Result<()> {
        // api 已不存在 signForFarm
        Ok(())
    }

    // 获取道具卡信息
    async fn get_card_info(&self) -> Result<CardInfo> {
        let body = json!({"version":18,"channel":1,"babelChannel":"121"});
        let data = self
            .request("myCardInfoForFarm", body.to_string().as_str())
            .await?;

        Ok(serde_json::from_value(data)?)
    }

    // 十次浇水任务
    async fn do_total_water_task(&self, task: TotalWaterTask) -> Result<()> {
        for _ in task.total_water_task_times..task.total_water_task_limit {
            let _ = self.water().await?;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        self.got_water_task_award("totalWaterTaskForFarm").await
    }

    // 领取浇水任务奖励
    async fn got_water_task_award(&self, function_id: &str) -> Result<()> {
        let res = self
            .request(
                function_id,
                r#"{"version":18,"channel":1,"babelChannel":"121"}"#,
            )
            .await?;

        match self.is_success(&res) {
            true => {
                let mut amount = res["amount"].as_u64().unwrap_or(0);
                if amount == 0 {
                    amount = res["totalWaterTaskEnergy"].as_u64().unwrap_or(0);
                }
                info!(
                    "{}, 成功领取浇水任务奖励, 获得水滴:{}g!",
                    self.account.name(),
                    amount
                );

                let can_do_pop_task = res["todayGotWaterGoalTask"]["canPop"]
                    .as_bool()
                    .unwrap_or(false);
                if can_do_pop_task {
                    let _ = self.do_pop_task().await;
                };
            }
            false => {
                info!("{}, 领取浇水任务奖励失败, {}", self.account.name(), res);
            }
        }

        Ok(())
    }

    // 获取签到领水页面数据
    async fn get_clock_in_data(&self) -> Result<Value> {
        // clockInitForFarm
        let data = self
            .request(
                "clockInInitForFarm",
                r#"{"version":18,"channel":3,"babelChannel":"10"}"#,
            )
            .await?;
        match self.is_success(&data) {
            true => Ok(data),
            false => Err(anyhow!(JError::ParseFailure)),
        }
    }

    // 获取签到领水页面任务
    async fn get_clock_in_task(&self, data: Option<Value>) -> Result<ClockInTask> {
        let data = match data {
            Some(data) => data,
            None => self.get_clock_in_data().await?,
        };
        Ok(serde_json::from_value(data).map_err(|_| JError::ParseFailure)?)
    }

    // 首次浇水任务
    async fn do_first_water_task(&self) -> Result<()> {
        let bool = self.water().await?;
        match bool {
            true => self.got_water_task_award("firstWaterTaskForFarm").await?,
            false => {
                info!("{}, 首次浇水任务失败.", self.account.name());
            }
        }
        Ok(())
    }

    // 从APP首页免费水果进入东东农场任务
    async fn do_treasure_box_task(&self, task: TreasureBoxTask) -> Result<()> {
        let body = json!({
            "type":1,
            "babelChannel":"121",
            "version":18,
            "channel":1
        });

        let _ = self
            .request("ddnc_getTreasureBoxAward", body.to_string().as_str())
            .await;

        tokio::time::sleep(Duration::from_secs(1)).await;

        let body = json!({
            "babelChannel":"10",
            "line": task.line,
            "channel":3,
            "type":2,
            "version":18});

        let res = self
            .request("ddnc_getTreasureBoxAward", body.to_string().as_str())
            .await?;

        match self.is_success(&res) {
            true => {
                let amount = res["waterGram"].as_u64().unwrap_or(0);
                info!(
                    "{}, 完成任务:《通过“免费水果”访问农场》, 获得水滴:{}g!",
                    self.account.name(),
                    amount
                );
            }
            false => {
                info!(
                    "{}, 无法完成任务:《通过“免费水果”访问农场》,{}",
                    self.account.name(),
                    res
                );
            }
        };
        Ok(())
    }

    // 浏览任务
    async fn do_browse_task(&self, task_list: Vec<BrowseTaskItem>) -> Result<()> {
        for task in task_list {
            if task.had_finished_times >= task.limit {
                info!(
                    "{}, 今日已完成任务《{}》!",
                    self.account.name(),
                    task.main_title
                );
                continue;
            }
            let data = json!({
                "babelChannel":"10",
                "advertId": task.advert_id,
                "type": 0,
                "channel":3,
                "version":18
            });

            let _ = self
                .request("browseAdTaskForFarm", data.to_string().as_str())
                .await;

            info!(
                "{}, 正在进行任务:《{}》, 等待{}秒...",
                self.account.name(),
                task.main_title,
                task.time
            );
            tokio::time::sleep(Duration::from_secs(task.time.into())).await;

            let data = json!({
                "babelChannel":"10",
                "advertId": task.advert_id,
                "type": 1,
                "channel":3,
                "version":18
            });
            let res = self
                .request("browseAdTaskForFarm", data.to_string().as_str())
                .await;
            if res.is_err() {
                info!(
                    "{}, 执行任务:《{}》失败.",
                    self.account.name(),
                    task.main_title
                );
                continue;
            }
            let data = res.unwrap();

            match self.is_success(&data) {
                true => {
                    let amount = data["amount"].as_u64().unwrap_or(0);
                    info!(
                        "{}, 执行任务:《{}》成功, 获得水滴:{}g!",
                        self.account.name(),
                        task.main_title,
                        amount
                    );
                    let can_do_pop_task = data["todayGotWaterGoalTask"]["canPop"]
                        .as_bool()
                        .unwrap_or(false);
                    if can_do_pop_task {
                        let _ = self.do_pop_task().await;
                    }
                }
                false => {
                    info!(
                        "{}, 执行任务:《{}》失败.",
                        self.account.name(),
                        task.main_title
                    );
                    continue;
                }
            }
        }
        Ok(())
    }

    // 水滴雨任务
    async fn do_water_rain_task(&self, task: WaterRainTask) -> Result<()> {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            * 1000;

        if time < task.last_time + 3 * 60 * 60 * 1000 {
            info!(
                "{}, 第{}次水滴雨任务未到时间!",
                self.account.name(),
                task.win_times + 1
            );
            return Ok(());
        }
        let body = json!({
            "type":1,
            "hongBaoTimes": time % 5 + 50,
            "version":14,
            "channel":1
        });
        let res = self
            .request("waterRainForFarm", body.to_string().as_str())
            .await?;

        match self.is_success(&res) {
            true => {
                let amount = res["addEnergy"].as_u64().unwrap_or(0);
                info!(
                    "{}, 成功完成第{}次水滴雨任务, 获得水滴:{}g!",
                    self.account.name(),
                    task.win_times + 1,
                    amount
                );
            }
            false => {
                info!(
                    "{:?}, 执行第{}次水滴雨任务失败.",
                    self.account.name(),
                    task.win_times + 1
                )
            }
        }
        Ok(())
    }

    // 为两位好友浇水任务
    async fn do_water_friend_task(&self, task: WaterFriendTask) -> Result<()> {
        if task.water_friend_count_key < task.water_friend_max {
            let url = format!(
                "{}?functionId=friendListInitForFarm&appid=wh5&client=iOS&clientVersion=11.2.8",
                self.base_url
            );
            let body = r#"{"lastId":null,"version":18,"channel":1,"babelChannel":"121"}"#;
            let data = self
                .client
                .post(url)
                .body(format!("body={:?}", body))
                .send()
                .await?
                .json::<Value>()
                .await
                .map_err(|_| JError::RequestFailure)?;
            let friends: FriendInfoList = serde_json::from_value(data)?;
            let mut count = task.water_friend_max - task.water_friend_count_key;

            for friend in friends.friends {
                if friend.friend_state == 0 {
                    continue;
                }
                let body = json!({
                    "shareCode": friend.share_code,
                    "version": 18,
                    "channel": 1,
                    "babelChannel": "121"
                });
                let _ = self
                    .request("waterFriendForFarm", body.to_string().as_str())
                    .await;
                count -= 1;
                if count == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }

            let res = self
                .request(
                    "waterFriendGotAwardForFarm",
                    r#"{"version":18,"channel":1,"babelChannel":"121"}"#,
                )
                .await?;

            match self.is_success(&res) {
                true => {
                    let amount = res["addWater"].as_u64().unwrap_or(0);
                    info!(
                        "{:?}, 成功领取任务:《为两位好友浇水》奖励, 获得水滴:{}g!",
                        self.account.name(),
                        amount
                    );
                }
                false => {
                    info!(
                        "{:?}, 领取任务:《为两位好友浇水》奖励失败!",
                        self.account.name()
                    );
                }
            }
        }

        Ok(())
    }

    // 签到领水->签到任务
    async fn do_clock_in_sign_in_task(&self) -> Result<()> {
        let body = json!({
            "version": 18,
            "channel": 1,
            "babelChannel": "121",
            "type": 1
        });
        let res = self
            .request("clockInForFarm", body.to_string().as_str())
            .await?;

        match self.is_success(&res) {
            true => {
                info!(
                    "{:?}, 成功完成任务:《签到领水->签到》, {:?}",
                    self.account.name(),
                    res
                );
                let card_info = self.get_card_info().await;
                if card_info.is_ok() && card_info.as_ref().unwrap().sign_card > 0 {
                    let use_num = match card_info.as_ref().unwrap().sign_card >= 3 {
                        true => 3,
                        false => card_info.unwrap().sign_card,
                    };
                    for _ in 0..use_num {
                        let _ = self.use_card("signCard", "加签卡").await;
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            false => {
                info!("{}, 任务:《签到领水->签到》执行失败!", self.account.name());
            }
        }
        Ok(())
    }

    // 签到领水->限时关注领水滴
    async fn do_clock_in_follow_task(&self, tasks: Vec<FollowTask>) -> Result<()> {
        for task in tasks {
            if task.had_got {
                continue;
            }

            if !task.had_follow {
                // 未关注
                let body = json!({
                    "id": task.id,
                    "babelChannel": "10",
                    "channel": 3,
                    "type": "theme",
                    "step":1,
                    "version":18
                });
                let _ = self
                    .request("clockInFollowForFarm", body.to_string().as_str())
                    .await;
                info!("{}, 关注《{}》!", self.account.name(), task.name);
            }
            let body = json!({"id": task.id,"babelChannel":"10","channel":3,"type":"theme","step":2,"version":18});
            let res = self
                .request("clockInFollowForFarm", body.to_string().as_str())
                .await?;
            match self.is_success(&res) {
                true => {
                    let amount = res["amount"].as_u64().unwrap_or(0);
                    info!(
                        "{}, 成功领取任务《关注{}》奖励, 获得水滴:{}g!",
                        self.account.name(),
                        task.name,
                        amount
                    );
                }
                false => {
                    info!(
                        "{}, 领取任务《关注{}》奖励失败!",
                        self.account.name(),
                        task.name
                    );
                }
            }
        }
        Ok(())
    }

    // 使用道具卡
    async fn use_card(&self, card_type: &str, card_name: &str) -> Result<()> {
        let body = json!({
            "cardType": card_type,
            "babelChannel":"10",
            "channel":3,
            "version":18
        });

        let res = self
            .request("userMyCardForFarm", body.to_string().as_str())
            .await?;
        match self.is_success(&res) {
            true => {
                info!("{}, 使用{}成功!", self.account.name(), card_name);
            }
            false => {
                info!("{}, 使用{}失败!", self.account.name(), card_name);
            }
        }
        Ok(())
    }

    // 领取浇水阶段性奖励
    // {"babelChannel":"10","channel":3,"type":4,"version":18} // 发芽
    // {"type":1,"version":18,"channel":1,"babelChannel":"121"} // 开花
    // {"type":3,"version":18,"channel":1,"babelChannel":"121"} // 结果
    async fn got_stage_award(&self) -> Result<()> {
        // let body = json!({"babelChannel":"10","channel":3,"type":1,"version":18});
        // let res = self
        //     .request("gotStageAwardForFarm", body.to_string().as_str())
        //     .await?;

        // match self.is_success(&res) {
        //     true => {
        //         let amount = res["addEnergy"].as_u64().unwrap_or(0);
        //         info!(
        //             "{}, 成功领取浇水阶段性奖励, 获得水滴:{}g!",
        //             self.account.name(),
        //             amount
        //         );
        //     }
        //     false => {
        //         info!("{}, 领取浇水阶段性奖励失败, {}", self.account.name(), res);
        //     }
        // }

        Ok(())
    }

    // 点击小鸭子
    async fn click_duck(&self) -> Result<()> {
        for i in 0..10 {
            let body = json!({"babelChannel":"10","channel":3,"type":2,"version":18});
            let res = self
                .request("getFullCollectionReward", body.to_string().as_str())
                .await?;
            match self.is_success(&res) {
                true => {
                    let title = res["title"].to_string();
                    info!(
                        "{}, 第{}次点鸭子成功, {}",
                        self.account.name(),
                        i + 1,
                        title
                    );
                }
                false => {
                    if res["code"].as_str().unwrap_or("999") == "10" {
                        info!("{}, 今日点鸭子次数已达上限!", self.account.name());
                        break;
                    } else {
                        info!(
                            "{}, 第{}次点击鸭子出错, {}!",
                            self.account.name(),
                            i + 1,
                            res
                        );
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Ok(())
    }

    // 获取可更换种植的的商品列表
    // getExchangeLevelList
    // {"version":18,"channel":3,"babelChannel":"10"}
    // async fn get_exchange_goods(&self) -> Result<()> {
    //     //
    //     Ok(())
    // }

    // 更换种植的商品
    // exchangeGood
    // {"afterSkuId":"100018093208","afterPrizeLevel":1,"babelChannel":"10","afterGoodsType":"qingjiebu5","channel":3,"version":18}
    // async fn exchange_goods(&self) -> Result<()> {
    //     Ok(())
    // }

    // 选择种植商品
    // choiceGoodsForFarm
    // {"afterSkuId":"100018093208","afterPrizeLevel":1,"babelChannel":"10","afterGoodsType":"qingjiebu5","channel":3,"version":18}
    // async fn choic_goods(&self) -> Result<()> {
    //     Ok(())
    // }

    // 三餐定时领水
    async fn got_three_meal(&self) -> Result<()> {
        let utc_time = Utc::now();
        let china_timezone = FixedOffset::east(8 * 3600);
        let cur_hour = utc_time.with_timezone(&china_timezone).hour();
        if cur_hour >= 21 || (9..11).contains(&cur_hour) || (14..17).contains(&cur_hour) {
            info!(
                "{:?}, 当前时间不在任务《定时领水》时间范围内!",
                self.account.name()
            );
        }
        let body = json!({"type":0,"version":18,"channel":1,"babelChannel":"121"});

        let res = self
            .request("gotThreeMealForFarm", body.to_string().as_str())
            .await?;
        match self.is_success(&res) {
            true => {
                let amount = res["amount"].as_u64().unwrap_or(0);
                info!(
                    "{}, 完成任务《定时领水》, 获得水滴:{}g!",
                    self.account.name(),
                    amount
                );
            }
            false => {
                info!("{}, 无法完成任务《定时领水》, {}", self.account.name(), res);
            }
        }

        Ok(())
    }

    // 功能入口
    pub async fn run(&self) -> Result<()> {
        let farm_data = match self.get_farm_data().await {
            Ok(data) => data,
            Err(e) => {
                info!("{}, {}", self.account.name(), e);
                return Ok(());
            }
        };

        let can_do_pop_task = farm_data["todayGotWaterGoalTask"]["canPop"]
            .as_bool()
            .unwrap_or(false);

        match self.get_farm_info(Some(farm_data)).await {
            Ok(farm_info) => {
                info!("{}: 奖品信息:\n\t奖品名称: {}\n\t奖品等级: {}\n\t剩余水滴(g): {}\n\t已浇水滴(g): {}\n\t还需浇水(g): {}",
                 self.account.name(),
                 farm_info.name,
                 farm_info.prize_level,
                 farm_info.total_energy,
                 farm_info.tree_energy,
                 farm_info.tree_total_energy - farm_info.tree_energy
                );
            }
            Err(e) => {
                info!("{}, {}", self.account.name(), e);
                return Ok(());
            }
        };

        match self.get_card_info().await {
            Ok(card) => {
                info!(
                    "{}, 背包信息: \n\t水滴换豆卡: {}\n\t快速浇水卡: {}\n\t水滴翻倍卡: {}\n\t加签卡: {}",
                    self.account.name(),
                    card.bean_card,
                    card.fast_card,
                    card.double_card,
                    card.sign_card,
                )
            }
            Err(e) => {
                info!("{}, 获取背包信息失败, {}", self.account.name(), e);
            }
        }

        if can_do_pop_task {
            let _ = self.do_pop_task().await;
        }

        let task_info = match self.get_task_info().await {
            Ok(info) => info,
            Err(e) => {
                info!("{}, 无法获取任务列表, {}", self.account.name(), e);
                return Ok(());
            }
        };

        if !task_info.sign_init.f {
            let _ = self.sign_in().await;
        } else {
            info!("{}, 今日已完成《签到》任务!", self.account.name());
        }

        if !task_info.got_three_meal_init.f {
            let _ = self.got_three_meal().await;
        } else {
            info!("{}, 今日已完成《定时领水》任务!", self.account.name());
        }

        if !task_info.treasure_box_init.f {
            let _ = self.do_treasure_box_task(task_info.treasure_box_init).await;
        } else {
            info!(
                "{}, 今日已完成《通过“免费水果”访问农场》任务!",
                self.account.name()
            );
        }

        if !task_info.got_browse_task_ad_init.f {
            let _ = self
                .do_browse_task(task_info.got_browse_task_ad_init.user_browse_task_ads)
                .await;
        } else {
            info!("{}, 今日已完成所有《浏览xxx》任务!", self.account.name());
        }

        if !task_info.water_rain_init.f {
            let _ = self.do_water_rain_task(task_info.water_rain_init).await;
        } else {
            info!("{}, 今日已完成《收集水滴雨》任务!", self.account.name());
        }

        if !task_info.water_friend_task_init.f {
            let _ = self
                .do_water_friend_task(task_info.water_friend_task_init)
                .await;
        } else {
            info!("{}, 今日已完成《为两位好友浇水》任务!", self.account.name());
        }

        let clock_in_task = self.get_clock_in_task(None).await?;
        if !clock_in_task.today_signed {
            let _ = self.do_clock_in_sign_in_task().await;
        } else {
            info!("{}, 今日已完成《签到领水->签到》任务!", self.account.name());
        }

        let _ = self.do_clock_in_follow_task(clock_in_task.themes).await;

        let _ = self.click_duck().await;

        if let Ok(farm_info) = self.get_farm_info(None).await {
            if let Ok(card_info) = self.get_card_info().await {
                if farm_info.total_energy >= 100 && card_info.double_card >= 1 {
                    let _ = self.use_card("doubleCard", "水滴翻倍卡").await;
                }
            }
        };

        if !task_info.first_water_init.f {
            let _ = self.do_first_water_task().await;
        } else {
            info!("{}, 今日已完成《首次浇水》任务!", self.account.name());
        }

        if !task_info.total_water_task_init.f {
            let _ = self
                .do_total_water_task(task_info.total_water_task_init)
                .await;
        } else {
            info!("{}, 今日已完成《十次浇水》任务!", self.account.name());
        }

        let _ = self.got_stage_award().await;

        if let Ok(farm_info) = self.get_farm_info(None).await {
            info!("{}: 奖品信息:\n\t奖品名称: {}\n\t奖品等级: {}\n\t剩余水滴(g): {}\n\t已浇水滴(g): {}\n\t还需浇水(g): {}",
            self.account.name(),
            farm_info.name,
            farm_info.prize_level,
            farm_info.total_energy,
            farm_info.tree_energy,
            farm_info.tree_total_energy - farm_info.tree_energy
           );
        };

        Ok(())
    }
}
