import re
import json
import argparse
from typing import List, Tuple, Dict, Any

class GCodeConverter:
    def __init__(self, scale_x: float = 20.0, scale_y: float = 20.0, 
                 offset_x: float = 800.0, offset_y: float = 1000.0,
                 flip_y: bool = True, remarkable_height: int = 2160,
                 debug: bool = False):
        """
        初始化G-code转换器
        
        参数:
            scale_x: X轴缩放因子
            scale_y: Y轴缩放因子
            offset_x: X轴偏移量
            offset_y: Y轴偏移量
            flip_y: 是否翻转Y轴 (G-code通常Y轴向上，而屏幕Y轴向下)
            remarkable_height: reMarkable屏幕高度，用于Y轴翻转计算
            debug: 是否输出调试信息
        """
        self.scale_x = scale_x
        self.scale_y = scale_y
        self.offset_x = offset_x
        self.offset_y = offset_y
        self.flip_y = flip_y
        self.remarkable_height = remarkable_height
        self.debug = debug
        
    def transform_point(self, x: float, y: float) -> Tuple[int, int]:
        """将G-code坐标转换为reMarkable屏幕坐标"""
        screen_x = int(x * self.scale_x + self.offset_x)
        
        if self.flip_y:
            # 翻转Y轴 (G-code通常Y轴向上，而屏幕Y轴向下)
            screen_y = int(self.remarkable_height - (y * self.scale_y + self.offset_y))
        else:
            screen_y = int(y * self.scale_y + self.offset_y)
            
        return (screen_x, screen_y)
    
    def convert_file(self, gcode_path: str, chars_path: str = None) -> Dict[str, Any]:
        """
        转换G-code文件为JSON格式，并关联汉字信息
        
        参数:
            gcode_path: G-code文件路径
            chars_path: 汉字列表文件路径（可选）
            
        返回:
            包含笔画数据和汉字信息的JSON对象
        """
        # 读取汉字列表（如果提供）
        characters = []
        if chars_path:
            try:
                with open(chars_path, 'r', encoding='utf-8') as f:
                    chars_text = f.read().strip()
                    characters = list(chars_text.replace(" ", ""))
                print(f"读取了 {len(characters)} 个汉字")
            except Exception as e:
                print(f"读取汉字文件时出错: {e}")
                print("将继续处理G-code，但不会关联汉字信息")
        
        # 处理G-code文件
        all_strokes = []  # 所有笔画
        current_stroke = []  # 当前笔画的点
        pen_down = False
        current_x = 0.0
        current_y = 0.0
        
        # G-code命令正则表达式
        g0_pattern = re.compile(r'G0\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        g1_pattern = re.compile(r'G1\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        
        # 读取整个G-code文件
        with open(gcode_path, 'r') as f:
            lines = f.readlines()
        
        # 处理G-code文件，提取所有笔画
        i = 0
        while i < len(lines):
            line = lines[i].strip()
            
            # 跳过注释和空行
            if line.startswith(';') or not line:
                i += 1
                continue
            
            # 检测笔的状态
            if line.startswith('M3'):  # M3 - 开启主轴/激光，表示笔放下
                pen_down = True
                # 确保当前点被添加到新笔画
                if current_stroke == [] and current_x != 0 and current_y != 0:
                    current_stroke.append(self.transform_point(current_x, current_y))
            
            elif line.startswith('M5'):  # M5 - 关闭主轴/激光，表示笔抬起
                if pen_down and current_stroke:
                    all_strokes.append(current_stroke.copy())
                    current_stroke = []
                pen_down = False
            
            # 处理快速移动 (G0) - 通常是笔抬起状态下移动到新位置
            g0_match = g0_pattern.match(line)
            if g0_match:
                current_x = float(g0_match.group(1))
                current_y = float(g0_match.group(2))
                # 在G0移动后不添加点，因为这通常是笔抬起状态
            
            # 处理线性移动 (G1) - 通常是笔放下状态下绘制
            g1_match = g1_pattern.match(line)
            if g1_match:
                current_x = float(g1_match.group(1))
                current_y = float(g1_match.group(2))
                
                # 如果笔是放下状态，添加点到当前笔画
                if pen_down:
                    current_stroke.append(self.transform_point(current_x, current_y))
            
            i += 1
        
        # 确保最后一个笔画被添加
        if pen_down and current_stroke:
            all_strokes.append(current_stroke)
        
        print(f"共识别出 {len(all_strokes)} 个笔画")
        
        # 如果有汉字列表，按照汉字数量分配笔画
        all_characters = []
        if characters:
            # 计算每个汉字的平均笔画数
            strokes_per_char = len(all_strokes) / len(characters)
            print(f"估计每个汉字平均有 {strokes_per_char:.2f} 个笔画")
            
            # 如果笔画数量远少于汉字数量，可能是识别有问题
            if len(all_strokes) < len(characters) / 2:
                print("警告：笔画数量远少于汉字数量，可能是识别有问题")
                # 将所有笔画作为一个汉字
                all_characters = [{
                    "strokes": all_strokes,
                    "character": characters[0] if characters else "未知"
                }]
            else:
                # 按照平均笔画数分配汉字
                for i in range(len(characters)):
                    start_idx = int(i * strokes_per_char)
                    end_idx = int((i + 1) * strokes_per_char)
                    
                    # 确保索引在有效范围内
                    if start_idx < len(all_strokes):
                        char_strokes = all_strokes[start_idx:min(end_idx, len(all_strokes))]
                        if char_strokes:  # 确保有笔画
                            char_info = {
                                "strokes": char_strokes,
                                "character": characters[i]
                            }
                            all_characters.append(char_info)
        else:
            # 如果没有汉字列表，将所有笔画作为一个汉字
            all_characters = [{
                "strokes": all_strokes,
                "character": "未知"
            }]
        
        # 创建JSON输出
        return {
            "characters": all_characters
        }

def main():
    parser = argparse.ArgumentParser(description='将G-code转换为reMarkable JSON格式，并关联汉字信息')
    parser.add_argument('gcode', help='输入G-code文件路径')
    parser.add_argument('--chars', help='输入汉字列表文件路径')
    parser.add_argument('output', help='输出JSON文件路径')
    parser.add_argument('--scale-x', type=float, default=20.0, help='X轴缩放因子')
    parser.add_argument('--scale-y', type=float, default=20.0, help='Y轴缩放因子')
    parser.add_argument('--offset-x', type=float, default=800.0, help='X轴偏移量')
    parser.add_argument('--offset-y', type=float, default=1000.0, help='Y轴偏移量')
    parser.add_argument('--no-flip-y', action='store_false', dest='flip_y', 
                        help='不翻转Y轴 (默认会翻转)')
    parser.add_argument('--debug', action='store_true', help='输出调试信息')
    parser.add_argument('--strokes-per-char', type=float, help='每个汉字的笔画数（如果指定，将覆盖自动计算）')
    
    args = parser.parse_args()
    
    converter = GCodeConverter(
        scale_x=args.scale_x,
        scale_y=args.scale_y,
        offset_x=args.offset_x,
        offset_y=args.offset_y,
        flip_y=args.flip_y,
        debug=args.debug
    )
    
    json_output = converter.convert_file(args.gcode, args.chars)
    
    # 保存JSON到文件
    with open(args.output, 'w', encoding='utf-8') as f:
        json.dump(json_output, f, ensure_ascii=False, indent=2)
    
    print(f"转换完成! 输出文件: {args.output}")
    print(f"共转换 {len(json_output['characters'])} 个汉字")

if __name__ == "__main__":
    main() 