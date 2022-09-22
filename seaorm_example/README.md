# sea-orm example

``` shell
mkdir data

docker run --privileged -v $PWD/data:/var/lib/mysql -p 4444:3306 -e MYSQL_DATABASE=public -e MYSQL_USER=user_a -e MYSQL_ROOT_PASSWORD=zan3Kie1 --name mysql_instance -d mysql:8.0.30-debian --character-set-server=utf8mb4 --collation-server=utf8mb4_general_ci
```
