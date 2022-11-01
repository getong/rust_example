use ftlog::{writer::file_split::Period, LevelFilter, LogBuilder};

use ftlog::*;

#[tokio::main]
async fn main() {
    // 完整用法
    // 配置logger
    let logger = LogBuilder::new()
        //这里可以定义自己的格式，时间格式暂时不可以自定义
        // .format(format)
        // a) 这里可以配置输出到文件
        .file(std::path::PathBuf::from("./current.log"))
        // b) 这里可以配置输出到文件，并且按指定间隔分割。这里导出的按天分割日志文件如current-20221024.log
        // 配置为按分钟分割时导出的日志文件如current-20221024T1428.log
        .file_split(std::path::PathBuf::from("./current.log"), Period::Day)
        // 如果既不配置输出文件 a)， 也不配置按指定间隔分割文件 b)，则默认输出到stderr
        // a) 和 b) 互斥，写在后面的生效，比如这里就是file_split生效
        .max_log_level(LevelFilter::Info)
        .build()
        .expect("logger build failed");
    // 初始化
    logger.init().expect("set logger failed");
    println!("Hello, world!");

    trace!("Hello world!");
    debug!("Hello world!");
    info!("Hello world!");
    warn!("Hello world!");
    error!("Hello world!");

    //在main最后加入flush，否则在程序结束时未写入的日志会丢失：

    ftlog::logger().flush();
}
