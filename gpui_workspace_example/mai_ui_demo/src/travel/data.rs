//! Static demo itinerary, independent of GPUI rendering.
pub(super) struct ScheduleItem {
  pub(super) time: &'static str,
  pub(super) name: &'static str,
  pub(super) detail: &'static str,
  pub(super) icon: &'static str,
}
pub(super) struct DaySchedule {
  pub(super) items: [ScheduleItem; 2],
  pub(super) route: &'static str,
}
pub(super) const DAYS: [DaySchedule; 4] = [
  DaySchedule {
    items: [
      ScheduleItem {
        time: "09:30",
        name: "大理古城漫步",
        detail: "约 2h · 历史文化",
        icon: "town",
      },
      ScheduleItem {
        time: "14:00",
        name: "洱海骑行",
        detail: "约 2.5h · 户外运动",
        icon: "bike",
      },
    ],
    route: "古城 → 洱海 · 约 5 公里，骑行约 40 分钟",
  },
  DaySchedule {
    items: [
      ScheduleItem {
        time: "08:30",
        name: "苍山感通索道",
        detail: "约 3h · 山野探索",
        icon: "map",
      },
      ScheduleItem {
        time: "15:00",
        name: "寂照庵品茶",
        detail: "约 1.5h · 慢享时光",
        icon: "town",
      },
    ],
    route: "苍山 → 寂照庵 · 山间漫步约 20 分钟",
  },
  DaySchedule {
    items: [
      ScheduleItem {
        time: "09:00",
        name: "喜洲古镇",
        detail: "约 2h · 白族文化",
        icon: "town",
      },
      ScheduleItem {
        time: "15:30",
        name: "双廊看日落",
        detail: "约 2h · 洱海风光",
        icon: "sun",
      },
    ],
    route: "喜洲 → 双廊 · 环海出行约 50 分钟",
  },
  DaySchedule {
    items: [
      ScheduleItem {
        time: "09:30",
        name: "古城早市",
        detail: "约 1.5h · 当地美食",
        icon: "town",
      },
      ScheduleItem {
        time: "14:00",
        name: "带着回忆返程",
        detail: "前往大理站 · 返程",
        icon: "map",
      },
    ],
    route: "古城 → 大理站 · 预留 1 小时交通时间",
  },
];
