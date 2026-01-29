import 'package:flutter/material.dart';
import 'dart:developer' as developer;
import 'dart:io';
import 'package:file_picker/file_picker.dart';
import 'package:path/path.dart' as path;
import 'zig_ffi.dart';

void main() {
  // Initialize Zig FFI
  final zigFFI = ZigFFI();

  // Call the add function from Zig
  final result = zigFFI.add(5, 3);

  // Log the result
  developer.log('Zig add(5, 3) = $result');

  runApp(MyApp(zigFFI: zigFFI));
}

class MyApp extends StatelessWidget {
  final ZigFFI zigFFI;

  const MyApp({super.key, required this.zigFFI});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Canvas Demo',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.blue),
        useMaterial3: true,
      ),
      home: MainScreen(zigFFI: zigFFI),
    );
  }
}

class MainScreen extends StatefulWidget {
  final ZigFFI zigFFI;

  const MainScreen({super.key, required this.zigFFI});

  @override
  State<MainScreen> createState() => _MainScreenState();
}

class _MainScreenState extends State<MainScreen> {
  final List<DrawingPoint> _points = [];
  Color _selectedColor = Colors.black;
  double _strokeWidth = 5.0;
  final TextEditingController _chatController = TextEditingController();
  final List<ChatMessage> _messages = [];

  String? _currentDirectory;
  List<FileSystemEntity> _mp4Files = [];
  String? _selectedFile;
  String? _videoInfo;

  @override
  void dispose() {
    _chatController.dispose();
    super.dispose();
  }

  void _sendMessage() {
    if (_chatController.text.trim().isEmpty) return;

    setState(() {
      _messages.add(ChatMessage(
        text: _chatController.text,
        isUser: true,
        timestamp: DateTime.now(),
      ));
      _chatController.clear();
    });
  }

  Future<void> _openDirectory() async {
    String? selectedDirectory = await FilePicker.platform.getDirectoryPath();

    if (selectedDirectory != null) {
      setState(() {
        _currentDirectory = selectedDirectory;
        _mp4Files = _scanForMp4Files(selectedDirectory);
        _selectedFile = null;
      });
    }
  }

  List<FileSystemEntity> _scanForMp4Files(String directoryPath) {
    final directory = Directory(directoryPath);
    final files = <FileSystemEntity>[];

    try {
      final entities = directory.listSync(recursive: true);
      for (var entity in entities) {
        if (entity is File && path.extension(entity.path).toLowerCase() == '.mp4') {
          files.add(entity);
        }
      }
      files.sort((a, b) => path.basename(a.path).compareTo(path.basename(b.path)));
    } catch (e) {
      developer.log('Error scanning directory: $e');
    }

    return files;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Canvas Editor'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
      ),
      body: Row(
        children: [
          // Left Panel - File Explorer
          _buildFileExplorer(),

          // Divider
          const VerticalDivider(width: 1, thickness: 1),

          // Middle Panel - Canvas
          Expanded(
            flex: 3,
            child: _buildCanvasPanel(),
          ),

          // Divider
          const VerticalDivider(width: 1, thickness: 1),

          // Right Panel - Chat Box
          _buildChatPanel(),
        ],
      ),
    );
  }

  Widget _buildFileExplorer() {
    return Container(
      width: 250,
      color: Colors.grey[100],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.all(12.0),
            decoration: BoxDecoration(
              color: Colors.grey[200],
              border: Border(bottom: BorderSide(color: Colors.grey[300]!)),
            ),
            child: const Row(
              children: [
                Icon(Icons.folder, size: 20),
                SizedBox(width: 8),
                Text(
                  'FILE EXPLORER',
                  style: TextStyle(
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(8.0),
            child: SizedBox(
              width: double.infinity,
              child: ElevatedButton.icon(
                onPressed: _openDirectory,
                icon: const Icon(Icons.folder_open, size: 18),
                label: const Text('Open Directory'),
                style: ElevatedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 12),
                ),
              ),
            ),
          ),
          if (_currentDirectory != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8.0),
              child: Text(
                'Dir: ${path.basename(_currentDirectory!)}',
                style: TextStyle(
                  fontSize: 11,
                  color: Colors.grey[600],
                  fontWeight: FontWeight.w500,
                ),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 8.0, vertical: 4.0),
            child: Divider(height: 1),
          ),
          Expanded(
            child: _mp4Files.isEmpty
                ? Center(
                    child: Text(
                      _currentDirectory == null
                          ? 'No directory selected'
                          : 'No .mp4 files found',
                      style: TextStyle(
                        color: Colors.grey[600],
                        fontSize: 12,
                      ),
                      textAlign: TextAlign.center,
                    ),
                  )
                : ListView.builder(
                    padding: const EdgeInsets.all(8.0),
                    itemCount: _mp4Files.length,
                    itemBuilder: (context, index) {
                      final file = _mp4Files[index];
                      final fileName = path.basename(file.path);
                      final isSelected = _selectedFile == file.path;

                      return _buildFileItem(
                        Icons.video_file,
                        fileName,
                        file.path,
                        isSelected,
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }

  Widget _buildFileItem(IconData icon, String name, String filePath, bool isSelected) {
    return Container(
      margin: const EdgeInsets.only(bottom: 2),
      decoration: BoxDecoration(
        color: isSelected ? Colors.blue[50] : Colors.transparent,
        borderRadius: BorderRadius.circular(4),
      ),
      child: ListTile(
        dense: true,
        leading: Icon(
          icon,
          size: 18,
          color: Colors.blue[700],
        ),
        title: Text(
          name,
          style: TextStyle(
            fontSize: 13,
            fontWeight: isSelected ? FontWeight.w600 : FontWeight.normal,
          ),
          maxLines: 2,
          overflow: TextOverflow.ellipsis,
        ),
        onTap: () {
          setState(() {
            _selectedFile = filePath;
            _videoInfo = null; // Reset video info
          });
          _loadVideoInfo(filePath);
        },
        selected: isSelected,
      ),
    );
  }

  void _loadVideoInfo(String filePath) {
    final info = widget.zigFFI.getVideoInformation(filePath);
    setState(() {
      _videoInfo = info;
      if (info != null) {
        _messages.add(ChatMessage(
          text: info,
          isUser: false,
          timestamp: DateTime.now(),
        ));
      } else {
        _messages.add(ChatMessage(
          text: 'Failed to load video information',
          isUser: false,
          timestamp: DateTime.now(),
        ));
      }
    });
  }

  Widget _buildCanvasPanel() {
    return Column(
      children: [
        // Toolbar
        Container(
          padding: const EdgeInsets.all(8.0),
          decoration: BoxDecoration(
            color: Colors.grey[200],
            border: Border(bottom: BorderSide(color: Colors.grey[300]!)),
          ),
          child: Column(
            children: [
              Row(
                children: [
                  IconButton(
                    icon: const Icon(Icons.clear),
                    onPressed: () {
                      setState(() {
                        _points.clear();
                      });
                    },
                    tooltip: 'Clear Canvas',
                  ),
                  const SizedBox(width: 16),
                  const Text('Colors: '),
                  const SizedBox(width: 8),
                  _buildColorButton(Colors.black),
                  _buildColorButton(Colors.red),
                  _buildColorButton(Colors.blue),
                  _buildColorButton(Colors.green),
                  _buildColorButton(Colors.yellow),
                  _buildColorButton(Colors.purple),
                  _buildColorButton(Colors.orange),
                ],
              ),
              Row(
                children: [
                  const Text('Stroke: '),
                  Expanded(
                    child: Slider(
                      value: _strokeWidth,
                      min: 1.0,
                      max: 20.0,
                      divisions: 19,
                      label: _strokeWidth.round().toString(),
                      onChanged: (value) {
                        setState(() {
                          _strokeWidth = value;
                        });
                      },
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        // Canvas
        Expanded(
          child: Container(
            margin: const EdgeInsets.all(16.0),
            decoration: BoxDecoration(
              border: Border.all(color: Colors.grey, width: 2),
              borderRadius: BorderRadius.circular(8),
              color: Colors.white,
            ),
            child: GestureDetector(
              onPanStart: (details) {
                setState(() {
                  _points.add(
                    DrawingPoint(
                      offset: details.localPosition,
                      color: _selectedColor,
                      strokeWidth: _strokeWidth,
                    ),
                  );
                });
              },
              onPanUpdate: (details) {
                setState(() {
                  _points.add(
                    DrawingPoint(
                      offset: details.localPosition,
                      color: _selectedColor,
                      strokeWidth: _strokeWidth,
                    ),
                  );
                });
              },
              onPanEnd: (details) {
                setState(() {
                  _points.add(DrawingPoint.end());
                });
              },
              child: CustomPaint(
                painter: CanvasPainter(points: _points),
                size: Size.infinite,
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildChatPanel() {
    return Container(
      width: 300,
      color: Colors.grey[50],
      child: Column(
        children: [
          Container(
            padding: const EdgeInsets.all(12.0),
            decoration: BoxDecoration(
              color: Colors.grey[200],
              border: Border(bottom: BorderSide(color: Colors.grey[300]!)),
            ),
            child: const Row(
              children: [
                Icon(Icons.chat_bubble_outline, size: 20),
                SizedBox(width: 8),
                Text(
                  'CHAT',
                  style: TextStyle(
                    fontWeight: FontWeight.bold,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView.builder(
              padding: const EdgeInsets.all(8.0),
              itemCount: _messages.length,
              itemBuilder: (context, index) {
                final message = _messages[index];
                return _buildChatBubble(message);
              },
            ),
          ),
          Container(
            padding: const EdgeInsets.all(8.0),
            decoration: BoxDecoration(
              color: Colors.white,
              border: Border(top: BorderSide(color: Colors.grey[300]!)),
            ),
            child: Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _chatController,
                    decoration: const InputDecoration(
                      hintText: 'Type a message...',
                      border: OutlineInputBorder(),
                      contentPadding: EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 8,
                      ),
                    ),
                    onSubmitted: (_) => _sendMessage(),
                    maxLines: null,
                  ),
                ),
                const SizedBox(width: 8),
                IconButton(
                  icon: const Icon(Icons.send),
                  onPressed: _sendMessage,
                  color: Colors.blue,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildChatBubble(ChatMessage message) {
    return Align(
      alignment: message.isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.symmetric(vertical: 4.0),
        padding: const EdgeInsets.all(12.0),
        constraints: const BoxConstraints(maxWidth: 250),
        decoration: BoxDecoration(
          color: message.isUser ? Colors.blue[100] : Colors.grey[200],
          borderRadius: BorderRadius.circular(12),
        ),
        child: Text(
          message.text,
          style: const TextStyle(fontSize: 14),
        ),
      ),
    );
  }

  Widget _buildColorButton(Color color) {
    return GestureDetector(
      onTap: () {
        setState(() {
          _selectedColor = color;
        });
      },
      child: Container(
        width: 32,
        height: 32,
        margin: const EdgeInsets.symmetric(horizontal: 4),
        decoration: BoxDecoration(
          color: color,
          shape: BoxShape.circle,
          border: Border.all(
            color: _selectedColor == color ? Colors.black : Colors.grey,
            width: _selectedColor == color ? 3 : 1,
          ),
        ),
      ),
    );
  }
}

class ChatMessage {
  final String text;
  final bool isUser;
  final DateTime timestamp;

  ChatMessage({
    required this.text,
    required this.isUser,
    required this.timestamp,
  });
}

class DrawingPoint {
  final Offset? offset;
  final Color color;
  final double strokeWidth;

  DrawingPoint({
    this.offset,
    required this.color,
    required this.strokeWidth,
  });

  DrawingPoint.end()
      : offset = null,
        color = Colors.black,
        strokeWidth = 1.0;
}

class CanvasPainter extends CustomPainter {
  final List<DrawingPoint> points;

  CanvasPainter({required this.points});

  @override
  void paint(Canvas canvas, Size size) {
    for (int i = 0; i < points.length - 1; i++) {
      if (points[i].offset != null && points[i + 1].offset != null) {
        final paint = Paint()
          ..color = points[i].color
          ..strokeWidth = points[i].strokeWidth
          ..strokeCap = StrokeCap.round;

        canvas.drawLine(
          points[i].offset!,
          points[i + 1].offset!,
          paint,
        );
      }
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) {
    return true;
  }
}
