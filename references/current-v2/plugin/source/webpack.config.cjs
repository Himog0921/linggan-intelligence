const path = require('path');
const CopyPlugin = require('copy-webpack-plugin');
const MiniCssExtractPlugin = require('mini-css-extract-plugin');
const TerserPlugin = require('terser-webpack-plugin');

module.exports = {
  entry: {
    content: './src/content/index.js',
    background: './src/background/index.js',
    popup: './src/popup/index.jsx',
    dashboard: './src/dashboard/index.jsx',
  },
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: '[name].js',
    chunkFilename: '[name].chunk.js',
    publicPath: '',
    clean: true,
  },
  module: {
    rules: [
      {
        test: /\.jsx?$/,
        exclude: /node_modules/,
        use: {
          loader: 'babel-loader',
          options: {
            presets: ['@babel/preset-react'],
          },
        },
      },
      {
        test: /\.css$/,
        use: [MiniCssExtractPlugin.loader, 'css-loader'],
      },
    ],
  },
  plugins: [
    new MiniCssExtractPlugin({
      filename: '[name].css',
    }),
    new CopyPlugin({
      patterns: [
        { from: 'manifest.json', to: 'manifest.json' },
        { from: 'src/popup/popup.html', to: 'popup.html' },
        { from: 'src/dashboard/dashboard.html', to: 'dashboard.html' },
        { from: 'src/popup/popup.css', to: 'popup.css' },
        { from: 'src/dashboard/dashboard.css', to: 'dashboard.css' },
        { from: 'src/themes/ac-ui/popup.css', to: 'themes/ac-ui/popup.css' },
        { from: 'src/injected', to: 'injected' },
        { from: 'src/assets', to: '.', globOptions: { ignore: ['**/.gitkeep'] } },
      ],
    }),
  ],
  resolve: {
    extensions: ['.js', '.jsx'],
  },
  optimization: {
    minimize: true,
    minimizer: [
      new TerserPlugin({
        terserOptions: {
          keep_fnames: /persistNavigatedTaskTabsSnapshot/,
        },
      }),
    ],
    splitChunks: {
      chunks(chunk) {
        return chunk.name !== 'background';
      },
      cacheGroups: {
        default: false,
        defaultVendors: false,
        vendor: {
          test: /[\\/]node_modules[\\/](react|react-dom|scheduler)[\\/]/,
          name: 'vendor',
          chunks(chunk) {
            return chunk.name !== 'background';
          },
        },
      },
    },
  },
  devtool: 'cheap-module-source-map',
};
