import re
import json
import argparse
import io
import sys

# 检测Python版本
PY3 = sys.version_info[0] == 3

class GCodeConverter:
    def __init__(self, scale_x=20.0, scale_y=20.0, 
                 offset_x=800.0, offset_y=1000.0,
                 flip_y=True, remarkable_height=2160):
        """
        初始化G-code转换器
        
        参数:
            scale_x: X轴缩放因子
            scale_y: Y轴缩放因子
            offset_x: X轴偏移量
            offset_y: Y轴偏移量
            flip_y: 是否翻转Y轴 (G-code通常Y轴向上，而屏幕Y轴向下)
            remarkable_height: reMarkable屏幕高度，用于Y轴翻转计算
        """
        self.scale_x = scale_x
        self.scale_y = scale_y
        self.offset_x = offset_x
        self.offset_y = offset_y
        self.flip_y = flip_y
        self.remarkable_height = remarkable_height
        
    def transform_point(self, x, y):
        """将G-code坐标转换为reMarkable屏幕坐标"""
        screen_x = int(x * self.scale_x + self.offset_x)
        
        if self.flip_y:
            # 翻转Y轴 (G-code通常Y轴向上，而屏幕Y轴向下)
            screen_y = int(self.remarkable_height - (y * self.scale_y + self.offset_y))
        else:
            screen_y = int(y * self.scale_y + self.offset_y)
            
        return (screen_x, screen_y)
    
    def convert_file(self, gcode_path, chars_path=None):
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
                if PY3:
                    with io.open(chars_path, 'r', encoding='utf-8') as f:
                        chars_text = f.read().strip()
                else:
                    with io.open(chars_path, 'r', encoding='utf-8') as f:
                        chars_text = f.read().strip()
                characters = list(chars_text.replace(" ", ""))
                print("读取了 {} 个汉字".format(len(characters)))
            except Exception as e:
                print("读取汉字文件时出错: {}".format(e))
                print("将继续处理G-code，但不会关联汉字信息")
        
        # 处理G-code文件
        all_strokes = []  # 所有笔画
        character_strokes = []  # 当前汉字的笔画
        current_stroke = []  # 当前笔画的点
        pen_down = False
        current_x = 0.0
        current_y = 0.0
        
        # 用于检测新汉字开始的标志
        # 在G-code中，通常一个完整的汉字由多个笔画组成，每个笔画以M5结束
        # 我们需要检测何时开始一个新的汉字
        character_count = 0
        last_position = None
        
        # G-code命令正则表达式
        g0_pattern = re.compile(r'G0\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        g1_pattern = re.compile(r'G1\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        
        with io.open(gcode_path, 'r') as f:
            lines = f.readlines()
            
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
                        character_strokes.append(current_stroke[:])
                        current_stroke = []
                    pen_down = False
                    
                    # 检查下一行是否是新汉字的开始
                    # 通常，一个新汉字以G0命令开始，表示移动到新位置
                    next_line_idx = i + 1
                    while next_line_idx < len(lines) and (lines[next_line_idx].strip().startswith(';') or not lines[next_line_idx].strip()):
                        next_line_idx += 1
                    
                    if next_line_idx < len(lines):
                        next_line = lines[next_line_idx].strip()
                        # 如果下一行是G0，并且后面跟着M3，那么这可能是新汉字的开始
                        if next_line.startswith('G0'):
                            # 查看再下一行是否是M3
                            next_next_line_idx = next_line_idx + 1
                            while next_next_line_idx < len(lines) and (lines[next_next_line_idx].strip().startswith(';') or not lines[next_next_line_idx].strip()):
                                next_next_line_idx += 1
                            
                            if next_next_line_idx < len(lines) and lines[next_next_line_idx].strip().startswith('M3'):
                                # 这是新汉字的开始，保存当前汉字的笔画
                                if character_strokes:
                                    char_info = {
                                        "strokes": character_strokes[:],
                                        "character": characters[character_count] if character_count < len(characters) else "未知_{}".format(character_count)
                                    }
                                    all_strokes.append(char_info)
                                    character_strokes = []
                                    character_count += 1
                
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
        
        # 确保最后一个笔画和汉字被添加
        if pen_down and current_stroke:
            character_strokes.append(current_stroke)
        
        if character_strokes:
            char_info = {
                "strokes": character_strokes[:],
                "character": characters[character_count] if character_count < len(characters) else "未知_{}".format(character_count)
            }
            all_strokes.append(char_info)
        
        # 创建JSON输出
        return {
            "characters": all_strokes
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
    
    args = parser.parse_args()
    
    converter = GCodeConverter(
        scale_x=args.scale_x,
        scale_y=args.scale_y,
        offset_x=args.offset_x,
        offset_y=args.offset_y,
        flip_y=args.flip_y
    )
    
    json_output = converter.convert_file(args.gcode, args.chars)
    
    # 保存JSON到文件
    if PY3:
        with io.open(args.output, 'w', encoding='utf-8') as f:
            json.dump(json_output, f, ensure_ascii=False, indent=2)
    else:
        with io.open(args.output, 'w', encoding='utf-8') as f:
            if isinstance(json_output, str):
                f.write(unicode(json.dumps(json_output, ensure_ascii=False, indent=2)))
            else:
                f.write(unicode(json.dumps(json_output, ensure_ascii=False, indent=2)))
    
    print("转换完成! 输出文件: {}".format(args.output))
    print("共转换 {} 个汉字".format(len(json_output["characters"])))

if __name__ == "__main__":
    main() 