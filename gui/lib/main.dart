import 'package:flutter/material.dart';
import 'dart:developer' as developer;
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'dart:async';
import 'package:flutter/foundation.dart';
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
      title: 'Video Frame Viewer',
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
  final TextEditingController _chatController = TextEditingController();
  final List<ChatMessage> _messages = [];

  String? _currentDirectory;
  List<FileSystemEntity> _mp4Files = [];
  String? _selectedFile;
  String? _videoInfo;
  ui.Image? _currentFrame;
  bool _isLoadingFrame = false;

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
        _currentFrame = null;
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
        title: const Text('Video Frame Viewer'),
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
            _videoInfo = null;
            _currentFrame = null;
          });
          _loadVideoInfo(filePath);
          _loadFirstFrame(filePath);
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

  Future<void> _loadFirstFrame(String filePath) async {
    setState(() {
      _isLoadingFrame = true;
    });

    try {
      final result = await compute(_extractFrameInIsolate, filePath);

      if (result != null) {
        final (data, width, height) = result;

        if (data == null) {
          setState(() {
            _isLoadingFrame = false;
            _messages.add(ChatMessage(
              text: 'Failed to extract frame data',
              isUser: false,
              timestamp: DateTime.now(),
            ));
          });
          return;
        }

        // Convert RGB24 data to RGBA
        final rgbaData = _convertRgbToRgba(data);

        // Create image from bytes
        final image = await _createImageFromPixels(rgbaData, width, height);

        setState(() {
          _currentFrame = image;
          _isLoadingFrame = false;
          _messages.add(ChatMessage(
            text: 'Loaded first frame: ${width}x$height',
            isUser: false,
            timestamp: DateTime.now(),
          ));
        });
      } else {
        setState(() {
          _isLoadingFrame = false;
          _messages.add(ChatMessage(
            text: 'Failed to load first frame',
            isUser: false,
            timestamp: DateTime.now(),
          ));
        });
      }
    } catch (e) {
      setState(() {
        _isLoadingFrame = false;
        _messages.add(ChatMessage(
          text: 'Error loading frame: $e',
          isUser: false,
          timestamp: DateTime.now(),
        ));
      });
    }
  }

  static (List<int>?, int, int)? _extractFrameInIsolate(String filePath) {
    try {
      final zigFFI = ZigFFI();
      return zigFFI.getNthFrame(filePath, 0);
    } catch (e) {
      return null;
    }
  }

  Future<ui.Image> _createImageFromPixels(
    List<int> rgbaData,
    int width,
    int height,
  ) {
    final completer = Completer<ui.Image>();
    ui.decodeImageFromPixels(
      Uint8List.fromList(rgbaData),
      width,
      height,
      ui.PixelFormat.rgba8888,
      completer.complete,
    );
    return completer.future;
  }

  List<int> _convertRgbToRgba(List<int> rgb) =>
      List.generate(
        (rgb.length ~/ 3) * 4,
        (i) {
          final pixelIndex = i ~/ 4;
          final componentIndex = i % 4;
          return componentIndex < 3
              ? rgb[pixelIndex * 3 + componentIndex]
              : 255;
        },
      );

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
          child: Row(
            children: [
              const Icon(Icons.videocam, size: 20),
              const SizedBox(width: 8),
              Text(
                _selectedFile != null
                    ? 'File: ${path.basename(_selectedFile!)}'
                    : 'No video selected',
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                ),
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
            child: _isLoadingFrame
                ? const Center(
                    child: CircularProgressIndicator(),
                  )
                : _currentFrame != null
                    ? CustomPaint(
                        painter: FramePainter(frame: _currentFrame!),
                        size: Size.infinite,
                      )
                    : Center(
                        child: Text(
                          'Select a video file to view its first frame',
                          style: TextStyle(
                            color: Colors.grey[600],
                            fontSize: 14,
                          ),
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
        child: SelectableText(
          message.text,
          style: const TextStyle(fontSize: 14),
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

class FramePainter extends CustomPainter {
  final ui.Image frame;

  FramePainter({required this.frame});

  @override
  void paint(Canvas canvas, Size size) {
    // Calculate scaling to fit frame within canvas while maintaining aspect ratio
    final frameWidth = frame.width.toDouble();
    final frameHeight = frame.height.toDouble();
    final canvasWidth = size.width;
    final canvasHeight = size.height;

    final scaleX = canvasWidth / frameWidth;
    final scaleY = canvasHeight / frameHeight;
    final scale = scaleX < scaleY ? scaleX : scaleY;

    final scaledWidth = frameWidth * scale;
    final scaledHeight = frameHeight * scale;

    final offsetX = (canvasWidth - scaledWidth) / 2;
    final offsetY = (canvasHeight - scaledHeight) / 2;

    final srcRect = Rect.fromLTWH(0, 0, frameWidth, frameHeight);
    final dstRect = Rect.fromLTWH(offsetX, offsetY, scaledWidth, scaledHeight);

    canvas.drawImageRect(frame, srcRect, dstRect, Paint());
  }

  @override
  bool shouldRepaint(covariant FramePainter oldDelegate) {
    return oldDelegate.frame != frame;
  }
}
