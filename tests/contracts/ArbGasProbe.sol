// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// Chỉ dùng để đo gas trong revm (B0). KHÔNG deploy lên chain.
interface IVault {
    function lock(bytes calldata data) external returns (bytes memory);
    function take(address currency, address to, uint256 amount) external;
    function sync(address currency) external;
    function settle() external payable returns (uint256);
}

interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function approve(address spender, uint256 amount) external returns (bool);
    function balanceOf(address a) external view returns (uint256);
}

interface IRouter {
    function swapExactTokensForTokensSupportingFeeOnTransferTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external;
}

interface IPair {
    function swap(uint256 amount0Out, uint256 amount1Out, address to, bytes calldata data) external;
}

contract ArbGasProbe {
    address public constant VAULT = 0x238a358808379702088667322f80aC48bAd5e6c4;
    address public constant ROUTER = 0x10ED43C718714eb63d5aA57B78B54704E256024E;

    function lockAcquired(bytes calldata data) external returns (bytes memory) {
        (address currency, uint256 amount, address token, address quoteSell) =
            abi.decode(data, (address, uint256, address, address));
        IVault(VAULT).take(currency, address(this), amount);
        IERC20(currency).approve(ROUTER, type(uint256).max);
        IERC20(token).approve(ROUTER, type(uint256).max);
        address[] memory buyPath = new address[](2);
        buyPath[0] = currency;
        buyPath[1] = token;
        IRouter(ROUTER).swapExactTokensForTokensSupportingFeeOnTransferTokens(
            amount, 0, buyPath, address(this), type(uint256).max
        );
        uint256 tok = IERC20(token).balanceOf(address(this));
        address[] memory sellPath = new address[](2);
        sellPath[0] = token;
        sellPath[1] = quoteSell;
        IRouter(ROUTER).swapExactTokensForTokensSupportingFeeOnTransferTokens(
            tok, 0, sellPath, address(this), type(uint256).max
        );
        IVault(VAULT).sync(currency);
        uint256 have = IERC20(currency).balanceOf(address(this));
        IERC20(currency).transfer(VAULT, have);
        IVault(VAULT).settle();
        return data;
    }

    function runLock(bytes calldata data) external {
        IVault(VAULT).lock(data);
    }

    function pancakeCall(address, uint256 amount0, uint256 amount1, bytes calldata data) external {
        (address token, address quote, uint256 repay) = abi.decode(data, (address, address, uint256));
        uint256 got = amount0 > 0 ? amount0 : amount1;
        IERC20(quote).approve(ROUTER, type(uint256).max);
        IERC20(token).approve(ROUTER, type(uint256).max);
        address[] memory buyPath = new address[](2);
        buyPath[0] = quote;
        buyPath[1] = token;
        IRouter(ROUTER).swapExactTokensForTokensSupportingFeeOnTransferTokens(
            got, 0, buyPath, address(this), type(uint256).max
        );
        uint256 tok = IERC20(token).balanceOf(address(this));
        address[] memory sellPath = new address[](2);
        sellPath[0] = token;
        sellPath[1] = quote;
        IRouter(ROUTER).swapExactTokensForTokensSupportingFeeOnTransferTokens(
            tok, 0, sellPath, address(this), type(uint256).max
        );
        IERC20(quote).transfer(msg.sender, repay);
    }
}
