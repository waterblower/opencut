import 'package:flutter/material.dart';
import 'dart:developer' as developer;
import 'zig_ffi.dart';

void main() {
  // Initialize Zig FFI
  final zigFFI = ZigFFI();

  // Call the add function from Zig
  final result = zigFFI.add(5, 3);

  // Log the result
  developer.log('Zig add(5, 3) = $result');
  print('Zig add(5, 3) = $result');

  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Canvas Demo',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(seedColor: Colors.blue),
        useMaterial3: true,
      ),
      home: const CanvasScreen(),
    );
  }
}

class CanvasScreen extends StatefulWidget {
  const CanvasScreen({super.key});

  @override
  State<CanvasScreen> createState() => _CanvasScreenState();
}

class _CanvasScreenState extends State<CanvasScreen> {
  final List<DrawingPoint> _points = [];
  Color _selectedColor = Colors.black;
  double _strokeWidth = 5.0;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Canvas Drawing'),
        backgroundColor: Theme.of(context).colorScheme.inversePrimary,
        actions: [
          IconButton(
            icon: const Icon(Icons.clear),
            onPressed: () {
              setState(() {
                _points.clear();
              });
            },
            tooltip: 'Clear Canvas',
          ),
        ],
      ),
      body: Column(
        children: [
          // Color Picker
          Container(
            padding: const EdgeInsets.all(8.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                _buildColorButton(Colors.black),
                _buildColorButton(Colors.red),
                _buildColorButton(Colors.blue),
                _buildColorButton(Colors.green),
                _buildColorButton(Colors.yellow),
                _buildColorButton(Colors.purple),
                _buildColorButton(Colors.orange),
              ],
            ),
          ),
          // Stroke Width Slider
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16.0),
            child: Row(
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
        width: 40,
        height: 40,
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
