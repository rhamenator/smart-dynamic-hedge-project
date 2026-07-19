#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <iomanip>
#include <iostream>
#include <limits>
#include <map>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace {

constexpr const char* kEngineVersion = "0.2.0";

enum class OptionType { Call, Put };
enum class ExerciseStyle { European, American };

struct Inputs {
    double spot = 100.0;
    double strike = 100.0;
    double rate = 0.045;
    double dividend_yield = 0.0;
    double volatility = 0.20;
    double days_to_expiry = 30.0;
    OptionType option_type = OptionType::Call;
    ExerciseStyle style = ExerciseStyle::American;
    int contracts = -1;                // signed: +long, -short
    double multiplier = 100.0;
    double current_shares = 0.0;
    int tree_steps = 500;
    double no_trade_band_shares = 2.0;
    bool json = false;
    bool self_test = false;
};

struct PriceGreeks {
    double price = 0.0;
    double european_price = 0.0;
    double early_exercise_premium = 0.0;
    double delta = 0.0;
    double gamma = 0.0;
    double vega_per_vol_point = 0.0;    // price change for +1 vol percentage point
    double theta_per_calendar_day = 0.0;
    double rho_per_rate_point = 0.0;    // price change for +1 rate percentage point
};

struct Result {
    Inputs in;
    PriceGreeks pg;
    double option_position_delta_shares = 0.0;
    double target_stock_shares = 0.0;
    double raw_trade_shares = 0.0;
    double recommended_trade_shares = 0.0;
    double stock_notional = 0.0;
    double gamma_pnl_for_1pct_move = 0.0;
    std::string action;
};

std::string to_string(OptionType t) {
    return t == OptionType::Call ? "call" : "put";
}

std::string to_string(ExerciseStyle s) {
    return s == ExerciseStyle::European ? "european" : "american";
}

double norm_cdf(double x) {
    return 0.5 * std::erfc(-x / std::sqrt(2.0));
}

double payoff(OptionType type, double spot, double strike) {
    return type == OptionType::Call ? std::max(spot - strike, 0.0)
                                    : std::max(strike - spot, 0.0);
}

void validate(const Inputs& in) {
    if (!(in.spot > 0.0) || !(in.strike > 0.0)) {
        throw std::invalid_argument("spot and strike must be positive");
    }
    if (!(in.volatility > 0.0 && in.volatility < 10.0)) {
        throw std::invalid_argument("volatility must be in (0, 10)");
    }
    if (!(in.days_to_expiry >= 0.0 && in.days_to_expiry <= 36500.0)) {
        throw std::invalid_argument("days-to-expiry must be in [0, 36500]");
    }
    if (!(in.multiplier > 0.0)) {
        throw std::invalid_argument("multiplier must be positive");
    }
    if (in.tree_steps < 10 || in.tree_steps > 20000) {
        throw std::invalid_argument("tree-steps must be in [10, 20000]");
    }
    if (in.no_trade_band_shares < 0.0) {
        throw std::invalid_argument("no-trade-band must not be negative");
    }
}

double european_price(const Inputs& in, double spot, double vol, double years,
                      double rate) {
    if (years <= 0.0) {
        return payoff(in.option_type, spot, in.strike);
    }
    const double sqrt_t = std::sqrt(years);
    const double d1 = (std::log(spot / in.strike) +
                       (rate - in.dividend_yield + 0.5 * vol * vol) * years) /
                      (vol * sqrt_t);
    const double d2 = d1 - vol * sqrt_t;
    const double df_r = std::exp(-rate * years);
    const double df_q = std::exp(-in.dividend_yield * years);
    if (in.option_type == OptionType::Call) {
        return spot * df_q * norm_cdf(d1) - in.strike * df_r * norm_cdf(d2);
    }
    return in.strike * df_r * norm_cdf(-d2) - spot * df_q * norm_cdf(-d1);
}

double binomial_american_price(const Inputs& in, double spot, double vol,
                               double years, double rate) {
    if (years <= 0.0) {
        return payoff(in.option_type, spot, in.strike);
    }

    const int n = in.tree_steps;
    const double dt = years / static_cast<double>(n);
    const double u = std::exp(vol * std::sqrt(dt));
    const double d = 1.0 / u;
    const double growth = std::exp((rate - in.dividend_yield) * dt);
    const double denominator = u - d;
    if (std::abs(denominator) < 1e-15) {
        return european_price(in, spot, vol, years, rate);
    }
    double p = (growth - d) / denominator;
    // An invalid risk-neutral probability means the requested discretization is
    // numerically unsuitable. Increasing steps normally fixes it; clamping would
    // silently fabricate a price, so fail instead.
    if (!(p >= 0.0 && p <= 1.0)) {
        throw std::runtime_error("invalid binomial probability; increase tree steps or check inputs");
    }
    const double disc = std::exp(-rate * dt);

    std::vector<double> values(static_cast<std::size_t>(n + 1));
    for (int i = 0; i <= n; ++i) {
        const double node_spot = spot * std::pow(u, 2.0 * i - n);
        values[static_cast<std::size_t>(i)] = payoff(in.option_type, node_spot, in.strike);
    }

    for (int step = n - 1; step >= 0; --step) {
        for (int i = 0; i <= step; ++i) {
            const double continuation = disc *
                ((1.0 - p) * values[static_cast<std::size_t>(i)] +
                 p * values[static_cast<std::size_t>(i + 1)]);
            const double node_spot = spot * std::pow(u, 2.0 * i - step);
            const double exercise = payoff(in.option_type, node_spot, in.strike);
            values[static_cast<std::size_t>(i)] = std::max(continuation, exercise);
        }
    }
    return values[0];
}

double model_price(const Inputs& in, double spot, double vol, double years,
                   double rate) {
    if (in.style == ExerciseStyle::European) {
        return european_price(in, spot, vol, years, rate);
    }
    return binomial_american_price(in, spot, vol, years, rate);
}

PriceGreeks calculate_price_greeks(const Inputs& in) {
    const double years = in.days_to_expiry / 365.0;
    PriceGreeks out;
    out.price = model_price(in, in.spot, in.volatility, years, in.rate);
    out.european_price = european_price(in, in.spot, in.volatility, years, in.rate);
    out.early_exercise_premium = std::max(0.0, out.price - out.european_price);

    if (years <= 0.0) {
        if (in.option_type == OptionType::Call) {
            out.delta = in.spot > in.strike ? 1.0 : (in.spot < in.strike ? 0.0 : 0.5);
        } else {
            out.delta = in.spot < in.strike ? -1.0 : (in.spot > in.strike ? 0.0 : -0.5);
        }
        return out;
    }

    // Finite differences make the same machinery work for both European and
    // American exercise. Bumps are intentionally deterministic and recorded by
    // version so recommendation replays remain reproducible.
    const double hs = std::max(0.01, in.spot * 1e-3);
    const double s_up = model_price(in, in.spot + hs, in.volatility, years, in.rate);
    const double s_dn = model_price(in, std::max(1e-8, in.spot - hs), in.volatility, years, in.rate);
    out.delta = (s_up - s_dn) / (2.0 * hs);
    out.gamma = (s_up - 2.0 * out.price + s_dn) / (hs * hs);

    const double vol_bump = std::min(0.01, in.volatility * 0.25);
    const double v_up = model_price(in, in.spot, in.volatility + vol_bump, years, in.rate);
    const double v_dn = model_price(in, in.spot, std::max(1e-6, in.volatility - vol_bump), years, in.rate);
    // Normalize to a 0.01 absolute volatility move.
    out.vega_per_vol_point = (v_up - v_dn) / (2.0 * vol_bump) * 0.01;

    const double one_day = 1.0 / 365.0;
    const double shorter = std::max(0.0, years - one_day);
    out.theta_per_calendar_day = model_price(in, in.spot, in.volatility, shorter, in.rate) - out.price;

    const double rate_bump = 0.01;
    const double r_up = model_price(in, in.spot, in.volatility, years, in.rate + rate_bump);
    const double r_dn = model_price(in, in.spot, in.volatility, years, in.rate - rate_bump);
    out.rho_per_rate_point = 0.5 * (r_up - r_dn);
    return out;
}

Result calculate(const Inputs& in) {
    validate(in);
    Result result;
    result.in = in;
    result.pg = calculate_price_greeks(in);
    result.option_position_delta_shares =
        static_cast<double>(in.contracts) * in.multiplier * result.pg.delta;
    result.target_stock_shares = -result.option_position_delta_shares;
    result.raw_trade_shares = result.target_stock_shares - in.current_shares;
    const bool inside_band = std::abs(result.raw_trade_shares) <= in.no_trade_band_shares;
    result.recommended_trade_shares = inside_band ? 0.0 : result.raw_trade_shares;
    result.action = inside_band ? "hold_inside_band" : "rebalance_preview";
    result.stock_notional = std::abs(result.recommended_trade_shares) * in.spot;
    const double one_pct_move = 0.01 * in.spot;
    result.gamma_pnl_for_1pct_move = 0.5 * result.pg.gamma * one_pct_move * one_pct_move *
        static_cast<double>(in.contracts) * in.multiplier;
    return result;
}

std::string json_number(double value) {
    if (!std::isfinite(value)) {
        return "null";
    }
    std::ostringstream os;
    os << std::setprecision(15) << value;
    return os.str();
}

std::string result_json(const Result& r) {
    std::ostringstream os;
    os << "{";
    os << "\"engine_version\":\"" << kEngineVersion << "\",";
    os << "\"inputs\":{";
    os << "\"spot\":" << json_number(r.in.spot) << ",";
    os << "\"strike\":" << json_number(r.in.strike) << ",";
    os << "\"rate\":" << json_number(r.in.rate) << ",";
    os << "\"dividend_yield\":" << json_number(r.in.dividend_yield) << ",";
    os << "\"volatility\":" << json_number(r.in.volatility) << ",";
    os << "\"days_to_expiry\":" << json_number(r.in.days_to_expiry) << ",";
    os << "\"option_type\":\"" << to_string(r.in.option_type) << "\",";
    os << "\"exercise_style\":\"" << to_string(r.in.style) << "\",";
    os << "\"contracts\":" << r.in.contracts << ",";
    os << "\"multiplier\":" << json_number(r.in.multiplier) << ",";
    os << "\"current_shares\":" << json_number(r.in.current_shares) << ",";
    os << "\"tree_steps\":" << r.in.tree_steps << ",";
    os << "\"base_no_trade_band_shares\":" << json_number(r.in.no_trade_band_shares);
    os << "},";
    os << "\"pricing\":{";
    os << "\"model_price\":" << json_number(r.pg.price) << ",";
    os << "\"european_price\":" << json_number(r.pg.european_price) << ",";
    os << "\"early_exercise_premium\":" << json_number(r.pg.early_exercise_premium);
    os << "},";
    os << "\"greeks\":{";
    os << "\"delta\":" << json_number(r.pg.delta) << ",";
    os << "\"gamma\":" << json_number(r.pg.gamma) << ",";
    os << "\"vega_per_vol_point\":" << json_number(r.pg.vega_per_vol_point) << ",";
    os << "\"theta_per_calendar_day\":" << json_number(r.pg.theta_per_calendar_day) << ",";
    os << "\"rho_per_rate_point\":" << json_number(r.pg.rho_per_rate_point);
    os << "},";
    os << "\"hedge\":{";
    os << "\"option_position_delta_shares\":" << json_number(r.option_position_delta_shares) << ",";
    os << "\"target_stock_shares\":" << json_number(r.target_stock_shares) << ",";
    os << "\"raw_trade_shares\":" << json_number(r.raw_trade_shares) << ",";
    os << "\"recommended_trade_shares\":" << json_number(r.recommended_trade_shares) << ",";
    os << "\"action\":\"" << r.action << "\",";
    os << "\"stock_notional\":" << json_number(r.stock_notional);
    os << "},";
    os << "\"risk\":{";
    os << "\"position_gamma_pnl_for_1pct_move\":" << json_number(r.gamma_pnl_for_1pct_move);
    os << "}";
    os << "}";
    return os.str();
}

void print_human(const Result& r) {
    std::cout << std::fixed << std::setprecision(6);
    std::cout << "Smart Dynamic Hedge deterministic core v" << kEngineVersion << "\n";
    std::cout << "PAPER/OBSERVE calculation only; this binary has no broker API.\n\n";
    std::cout << "Option: " << r.in.contracts << " x " << to_string(r.in.style) << " "
              << to_string(r.in.option_type) << ", multiplier " << r.in.multiplier << "\n";
    std::cout << "Model price:                 " << r.pg.price << "\n";
    std::cout << "European comparison:         " << r.pg.european_price << "\n";
    std::cout << "Early-exercise premium:      " << r.pg.early_exercise_premium << "\n";
    std::cout << "Delta / gamma:               " << r.pg.delta << " / " << r.pg.gamma << "\n";
    std::cout << "Vega per vol point:          " << r.pg.vega_per_vol_point << "\n";
    std::cout << "Theta per calendar day:      " << r.pg.theta_per_calendar_day << "\n";
    std::cout << "Target stock shares:         " << r.target_stock_shares << "\n";
    std::cout << "Current stock shares:        " << r.in.current_shares << "\n";
    std::cout << "Raw hedge trade:             " << r.raw_trade_shares << "\n";
    std::cout << "Base no-trade band:          +/-" << r.in.no_trade_band_shares << "\n";
    std::cout << "Recommendation:              " << r.action << "\n";
    std::cout << "Paper trade preview shares:  " << r.recommended_trade_shares << "\n";
    std::cout << "Paper trade notional:        " << r.stock_notional << "\n";
}

void usage(const char* argv0) {
    std::cout
        << "Usage: " << argv0 << " [options]\n\n"
        << "  --spot N                    underlying spot\n"
        << "  --strike N                  option strike\n"
        << "  --rate N                    continuously compounded annual rate\n"
        << "  --dividend-yield N          continuous annual dividend yield\n"
        << "  --vol N                     annualized volatility as decimal\n"
        << "  --days N                    calendar days to expiry\n"
        << "  --type call|put\n"
        << "  --style european|american\n"
        << "  --contracts N               signed contracts (+long, -short)\n"
        << "  --multiplier N              units of underlying per contract\n"
        << "  --current-shares N\n"
        << "  --tree-steps N\n"
        << "  --no-trade-band N           base band in shares\n"
        << "  --json                      emit one JSON object\n"
        << "  --self-test                 run numerical smoke tests\n"
        << "  --help\n";
}

double parse_double(const std::string& name, const char* text) {
    char* end = nullptr;
    const double value = std::strtod(text, &end);
    if (end == text || *end != '\0' || !std::isfinite(value)) {
        throw std::invalid_argument("invalid numeric value for " + name + ": " + text);
    }
    return value;
}

int parse_int(const std::string& name, const char* text) {
    char* end = nullptr;
    const long value = std::strtol(text, &end, 10);
    if (end == text || *end != '\0' || value < std::numeric_limits<int>::min() ||
        value > std::numeric_limits<int>::max()) {
        throw std::invalid_argument("invalid integer value for " + name + ": " + text);
    }
    return static_cast<int>(value);
}

Inputs parse_args(int argc, char** argv) {
    Inputs in;
    for (int i = 1; i < argc; ++i) {
        const std::string arg = argv[i];
        auto next = [&]() -> const char* {
            if (i + 1 >= argc) throw std::invalid_argument("missing value after " + arg);
            return argv[++i];
        };
        if (arg == "--spot") in.spot = parse_double(arg, next());
        else if (arg == "--strike") in.strike = parse_double(arg, next());
        else if (arg == "--rate") in.rate = parse_double(arg, next());
        else if (arg == "--dividend-yield") in.dividend_yield = parse_double(arg, next());
        else if (arg == "--vol") in.volatility = parse_double(arg, next());
        else if (arg == "--days") in.days_to_expiry = parse_double(arg, next());
        else if (arg == "--contracts") in.contracts = parse_int(arg, next());
        else if (arg == "--multiplier") in.multiplier = parse_double(arg, next());
        else if (arg == "--current-shares") in.current_shares = parse_double(arg, next());
        else if (arg == "--tree-steps") in.tree_steps = parse_int(arg, next());
        else if (arg == "--no-trade-band") in.no_trade_band_shares = parse_double(arg, next());
        else if (arg == "--type") {
            const std::string value = next();
            if (value == "call") in.option_type = OptionType::Call;
            else if (value == "put") in.option_type = OptionType::Put;
            else throw std::invalid_argument("--type must be call or put");
        } else if (arg == "--style") {
            const std::string value = next();
            if (value == "european") in.style = ExerciseStyle::European;
            else if (value == "american") in.style = ExerciseStyle::American;
            else throw std::invalid_argument("--style must be european or american");
        } else if (arg == "--json") in.json = true;
        else if (arg == "--self-test") in.self_test = true;
        else if (arg == "--help" || arg == "-h") {
            usage(argv[0]);
            std::exit(0);
        } else {
            throw std::invalid_argument("unknown option: " + arg);
        }
    }
    return in;
}

void assert_near(double got, double expected, double tolerance, const char* what) {
    if (!std::isfinite(got) || std::abs(got - expected) > tolerance) {
        std::ostringstream os;
        os << what << " expected " << expected << " +/- " << tolerance << ", got " << got;
        throw std::runtime_error(os.str());
    }
}

void run_self_tests() {
    Inputs euro;
    euro.style = ExerciseStyle::European;
    euro.spot = 100.0;
    euro.strike = 100.0;
    euro.rate = 0.05;
    euro.dividend_yield = 0.0;
    euro.volatility = 0.20;
    euro.days_to_expiry = 365.0;
    euro.option_type = OptionType::Call;
    euro.contracts = -1;
    const Result call = calculate(euro);
    assert_near(call.pg.price, 10.4505836, 2e-5, "Black-Scholes call");
    assert_near(call.pg.delta, 0.63683, 5e-4, "call delta");
    if (!(call.target_stock_shares > 0.0)) {
        throw std::runtime_error("short-call hedge should require long stock");
    }

    Inputs put = euro;
    put.option_type = OptionType::Put;
    const double ep = calculate(put).pg.price;
    put.style = ExerciseStyle::American;
    put.tree_steps = 1000;
    const double ap = calculate(put).pg.price;
    if (ap + 1e-8 < ep) {
        throw std::runtime_error("American put must not be cheaper than European put");
    }

    Inputs dividend_call = euro;
    dividend_call.style = ExerciseStyle::American;
    dividend_call.dividend_yield = 0.08;
    dividend_call.days_to_expiry = 90.0;
    dividend_call.tree_steps = 1200;
    const Result dc = calculate(dividend_call);
    if (!(dc.pg.price >= dc.pg.european_price - 1e-8)) {
        throw std::runtime_error("American call comparison failed");
    }

    std::cout << "self-test: PASS\n";
}

} // namespace

int main(int argc, char** argv) {
    try {
        const Inputs in = parse_args(argc, argv);
        if (in.self_test) {
            run_self_tests();
            return 0;
        }
        const Result result = calculate(in);
        if (in.json) std::cout << result_json(result) << '\n';
        else print_human(result);
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "error: " << e.what() << '\n';
        return 2;
    }
}
