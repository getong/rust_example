use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IComet {
        function supply(address asset, uint amount) external;
        function supplyTo(address dst, address asset, uint amount) external;
        function supplyFrom(address from, address dst, address asset, uint amount) external;

        function withdraw(address asset, uint amount) external;
        function withdrawTo(address to, address asset, uint amount) external;
        function withdrawFrom(address src, address to, address asset, uint amount) external;

        function accrueAccount(address account) external;
        function borrowBalanceOf(address account) external view returns (uint256);
        function collateralBalanceOf(address account, address asset) external view returns (uint128);

        function getSupplyRate(uint utilization) external view returns (uint64);
        function getBorrowRate(uint utilization) external view returns (uint64);
        function getUtilization() external view returns (uint);

        function totalSupply() external view returns (uint256);
        function totalBorrow() external view returns (uint256);
        function balanceOf(address owner) external view returns (uint256);

        function baseToken() external view returns (address);
        function baseTokenPriceFeed() external view returns (address);
        function numAssets() external view returns (uint8);
        function getAssetInfo(uint8 i) external view returns (AssetInfo memory);
        function getAssetInfoByAddress(address asset) external view returns (AssetInfo memory);
        function getPrice(address priceFeed) external view returns (uint256);

        struct AssetInfo {
            uint8 offset;
            address asset;
            address priceFeed;
            uint64 scale;
            uint64 borrowCollateralFactor;
            uint64 liquidateCollateralFactor;
            uint64 liquidationFactor;
            uint128 supplyCap;
        }
    }

    #[sol(rpc)]
    interface IERC20 {
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function transfer(address to, uint256 amount) external returns (bool);
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address from, address to, uint256 amount) external returns (bool);
    }
}
