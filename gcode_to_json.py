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
        all_characters = []  # 所有汉字
        character_strokes = []  # 当前汉字的笔画
        current_stroke = []  # 当前笔画的点
        pen_down = False
        current_x = 0.0
        current_y = 0.0
        
        # 用于检测新汉字开始的标志
        character_count = 0
        stroke_count = 0
        
        # 记录上一个笔画的位置，用于检测汉字边界
        last_stroke_end_x = None
        last_stroke_end_y = None
        
        # G-code命令正则表达式
        g0_pattern = re.compile(r'G0\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        g1_pattern = re.compile(r'G1\s+X([+-]?[0-9]*\.?[0-9]+)Y([+-]?[0-9]*\.?[0-9]+)')
        
        # 读取整个G-code文件
        with open(gcode_path, 'r') as f:
            lines = f.readlines()
        
        # 首先分析文件，找出所有的G0命令，用于确定可能的汉字边界
        g0_positions = []
        for i, line in enumerate(lines):
            line = line.strip()
            if line.startswith('G0'):
                g0_match = g0_pattern.match(line)
                if g0_match:
                    x = float(g0_match.group(1))
                    y = float(g0_match.group(2))
                    g0_positions.append((i, x, y))
        
        if self.debug:
            print(f"找到 {len(g0_positions)} 个G0命令")
        
        # 分析G0命令之间的距离，找出可能的汉字边界
        # 通常，同一个汉字内的笔画之间的距离较小，不同汉字之间的距离较大
        if len(g0_positions) > 1:
            distances = []
            for i in range(1, len(g0_positions)):
                prev_x, prev_y = g0_positions[i-1][1], g0_positions[i-1][2]
                curr_x, curr_y = g0_positions[i][1], g0_positions[i][2]
                distance = ((curr_x - prev_x) ** 2 + (curr_y - prev_y) ** 2) ** 0.5
                distances.append((i, distance))
            
            # 按距离排序
            distances.sort(key=lambda x: x[1], reverse=True)
            
            # 取距离最大的前N个作为可能的汉字边界
            # N可以根据预期的汉字数量调整
            potential_boundaries = set()
            boundary_count = min(len(characters), len(distances) // 2) if characters else len(distances) // 10
            
            if self.debug:
                print(f"预计有 {boundary_count} 个汉字边界")
            
            for i in range(min(boundary_count, len(distances))):
                potential_boundaries.add(distances[i][0])
            
            if self.debug:
                print(f"找到 {len(potential_boundaries)} 个可能的汉字边界")
                print(f"边界索引: {sorted(potential_boundaries)}")
        
        # 处理G-code文件
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
                    character_strokes.append(current_stroke.copy())
                    stroke_count += 1
                    current_stroke = []
                    
                    # 记录笔画结束位置
                    last_stroke_end_x = current_x
                    last_stroke_end_y = current_y
                
                pen_down = False
                
                # 检查是否是汉字边界
                next_line_idx = i + 1
                while next_line_idx < len(lines) and (lines[next_line_idx].strip().startswith(';') or not lines[next_line_idx].strip()):
                    next_line_idx += 1
                
                # 检查是否到达文件末尾
                if next_line_idx >= len(lines):
                    # 文件结束，保存当前汉字
                    if character_strokes:
                        char_info = {
                            "strokes": character_strokes.copy(),
                            "character": characters[character_count] if character_count < len(characters) else f"未知_{character_count}"
                        }
                        all_characters.append(char_info)
                        character_count += 1
                        character_strokes = []
                else:
                    # 检查下一个非空行
                    next_line = lines[next_line_idx].strip()
                    
                    # 如果下一行是G0，检查是否是汉字边界
                    if next_line.startswith('G0'):
                        g0_match = g0_pattern.match(next_line)
                        if g0_match:
                            next_x = float(g0_match.group(1))
                            next_y = float(g0_match.group(2))
                            
                            # 计算与上一个笔画结束位置的距离
                            if last_stroke_end_x is not None and last_stroke_end_y is not None:
                                distance = ((next_x - last_stroke_end_x) ** 2 + (next_y - last_stroke_end_y) ** 2) ** 0.5
                                
                                # 如果距离较大，或者是预先确定的边界，认为是新汉字的开始
                                is_boundary = False
                                
                                # 检查是否是预先确定的边界
                                if len(g0_positions) > 1:
                                    for pos_idx, pos_x, pos_y in g0_positions:
                                        if pos_idx == next_line_idx and next_line_idx in potential_boundaries:
                                            is_boundary = True
                                            break
                                
                                # 如果距离超过阈值，也认为是边界
                                # 阈值可以根据实际情况调整
                                if distance > 5.0:  # 假设5.0是一个合理的阈值
                                    is_boundary = True
                                
                                if is_boundary and character_strokes:
                                    # 这是新汉字的开始，保存当前汉字的笔画
                                    char_info = {
                                        "strokes": character_strokes.copy(),
                                        "character": characters[character_count] if character_count < len(characters) else f"未知_{character_count}"
                                    }
                                    all_characters.append(char_info)
                                    character_count += 1
                                    character_strokes = []
                                    
                                    if self.debug:
                                        print(f"在行 {next_line_idx} 处检测到汉字边界，距离: {distance:.2f}")
            
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
            stroke_count += 1
        
        if character_strokes:
            char_info = {
                "strokes": character_strokes.copy(),
                "character": characters[character_count] if character_count < len(characters) else f"未知_{character_count}"
            }
            all_characters.append(char_info)
            character_count += 1
        
        print(f"共识别出 {stroke_count} 个笔画，{character_count} 个汉字")
        
        # 如果没有识别出任何汉字，但有笔画，则将所有笔画作为一个汉字
        if character_count == 0 and stroke_count > 0:
            print("警告：未能识别出任何汉字边界，将所有笔画作为单个汉字处理")
            
            # 尝试按固定数量的笔画分割
            if characters and stroke_count > len(characters):
                # 估计每个汉字的平均笔画数
                avg_strokes_per_char = stroke_count / len(characters)
                print(f"估计每个汉字平均有 {avg_strokes_per_char:.2f} 个笔画")
                
                # 按照平均笔画数分割
                all_strokes_flat = []
                for strokes in character_strokes:
                    all_strokes_flat.append(strokes)
                
                all_characters = []
                for i in range(len(characters)):
                    start_idx = int(i * avg_strokes_per_char)
                    end_idx = int((i + 1) * avg_strokes_per_char)
                    if start_idx < len(all_strokes_flat):
                        char_strokes = all_strokes_flat[start_idx:min(end_idx, len(all_strokes_flat))]
                        if char_strokes:
                            char_info = {
                                "strokes": char_strokes,
                                "character": characters[i]
                            }
                            all_characters.append(char_info)
            else:
                # 如果无法估计，则将所有笔画作为一个汉字
                all_characters = [{
                    "strokes": character_strokes,
                    "character": characters[0] if characters else "未知"
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