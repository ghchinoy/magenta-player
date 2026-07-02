# Magenta Swift Player Project

A Swift-based UI player for Magenta RealTime built on top of the existing C++ inference engine. This project demonstrates how to use Swift and SwiftUI to create a native macOS interface for real-time music generation.

## Overview

This project provides a complete Swift implementation that integrates with the [Magenta RealTime](https://magenta.withgoogle.com/mrt2) C++ inference engine. It includes:

- Native macOS application using SwiftUI
- Integration with `magentart::core::RealtimeRunner` for real-time music generation
- Real-time parameter control via sliders and toggles
- Audio visualization with level meters
- Model management (load/save presets)
- MIDI input support for note triggering
- Automatic audio file preloading

## Architecture

The Swift player follows a layered architecture pattern:

### 1. View Layer (`src/Views/`)
SwiftUI components for the UI including:
- Main player interface with controls
- Parameter sliders and toggles
- MIDI manager for input handling
- Error handling and alerts

### 2. Model Layer (`src/Models.swift`)
Data models representing:
- Audio engine metrics and performance data
- Parameter values for the generation engine
- Player state and configuration

### 3. Manager Layer (`src/Managers.swift`)
Business logic including:
- AudioEngineManager for AVAudioEngine integration
- PlayerManager for player state and engine interaction
- MIDIManager for CoreMIDI communication

### 4. Main View (`src/PlayerView.swift`)
Entry point and integration point that connects all layers

## How It Works

The Swift player integrates with the Magenta C++ inference engine (`magentart::core::RealtimeRunner`) through a C interface, enabling:

1. **Audio Flow**: The C++ inference engine runs on an inference thread, generating audio that flows to the Swift audio player via AVAudioEngine
2. **Parameter Synchronization**: Parameters are sent bidirectionally between Swift UI controls and the C++ engine
3. **MIDI Integration**: CoreMIDI handles virtual and hardware MIDI input, triggering notes in the C++ engine
4. **Real-time Updates**: Swift UI updates continuously based on engine metrics like audio levels and performance

## Building and Running

### Prerequisites

- Xcode 14.0 or later
- macOS 12 or later
- Apple Silicon Mac (M1+, Air/Max depending on model size)
- Magenta RealTime Xcode project with the C++ core library

### Building Steps

1. **Set up the Magenta RealTime project**: Build the C++ core library first
2. **Add this Swift package**: Integrate the Swift player into your Xcode workspace
3. **Configure dependencies**: Set up Swift Package Manager dependencies
4. **Build and run**: Run the application on Apple Silicon

### Building Commands

```bash
# Build the Magenta RealTime C++ core library
cmake . -B build
cmake --build build --target standalone

# For Swift package integration:
# Add this directory as a Swift Package in Xcode
# Configure build settings appropriately
```

## Key Links

- **Magenta RealTime Repository**: https://github.com/magenta/magenta-realtime
- **Magenta RealTime Website**: https://magenta.withgoogle.com/mrt2
- **Full Documentation**: https://magenta.github.io/magenta-realtime/

## Integration with Magenta RealTime

The Swift player is designed to work seamlessly with Magenta RealTime:

### Supported Model Sizes

- **`mrt2_small` (230M parameters)**: Runs real-time on any Apple Silicon Mac, including Air models
- **`mrt2_base` (2.4B parameters)**: Higher quality; requires a Pro Max chip for real-time streaming

### Hardware Support Matrix

| Device | `mrt2_small` (230M) | `mrt2_base` (2.4B) |
|--------|---------------------|---------------------|
| M5 Max | ✅ | ✅ |
| M3 Max | ✅ | ✅ |
| M2 Max | ✅ | ✅ |
| M4 Pro | ✅ | ✅ |
| M2 Pro | ✅ | ❌ |
| M1 Pro | ✅ | ❌ |
| M4 Air | ✅ | ❌ |
| M3 Air | ✅ | ❌ |
| M1 Air | ✅ | ❌ |

### Components

#### 1. SwiftUI Player Interface
- Native macOS UI with SwiftUI
- Visual consistency with Magenta's existing React UI
- Real-time parameter control (temperature, topk, volume, etc.)
- Audio level visualization
- Model management (load/save presets)

#### 2. Audio Engine Integration
- Uses `AVAudioEngine` with `AVAudioSourceNode` for zero-latency audio
- Audio format matching (48kHz, stereo, 32-bit float)
- Thread-safe communication with C++ engine
- Proper audio thread handling to avoid dropouts

#### 3. Parameter Management
- Bidirectional parameter sync between Swift and C++
- Advanced controls (temperature, topk, cfg_*, etc.)
- Real-time updates from engine back to UI
- Persistent parameter storage across sessions

#### 4. MIDI Integration
- CoreMIDI virtual destination for other app integration
- Hardware MIDI support for physical controllers
- Note triggering and timing control
- Audio level updates based on MIDI activity

#### 5. State Management
- Player state (playing, model loaded, error states)
- Engine metrics (performance, buffer levels, dropped frames)
- Bank management (save/load model states)
- Prompt surface (2D prompt blending)

## Development Experience

### Development Approach

This player was developed following the "building-by-worth" methodology:

1. **Documenting lessons learned** as we went through the process
2. **Creating modular, testable components** following Swift best practices
3. **Maintaining a consistent visual language** with the existing Magenta UI
4. **Integrating seamlessly with the existing C++ engine**

### Key Design Decisions

#### Language Integration
- **Separation of Concerns**: Swift for UI and business logic, C++ for high-performance audio processing
- **Strict Type Safety**: Swift's strong typing catches errors at compile-time
- **Performance Optimization**: Critical audio processing stays in C++ for speed

#### Audio Architecture
- **Zero-Latency Design**: `AVAudioSourceNode` for minimal audio latency
- **Format Matching**: Exact audio format matching (48kHz, stereo, 32-bit float)
- **Threading**: Proper separation between UI and audio threads

#### UI/UX Design
- **Visual Consistency**: Maintains Magenta's color scheme and typography
- **Component Reusability**: Design supports both AUv3 and Standalone use cases
- **Accessibility**: Proper voice-over support and keyboard navigation

### Technical Challenges and Solutions

#### 1. Swift-C++ Integration
**Challenge**: Seamless data transfer between Swift and C++ requires careful design.

**Solution**: 
- Used C interface for type-safe communication
- Designed data structures for efficient zero-copy transfer
- Implemented proper threading to avoid performance bottlenecks

#### 2. Real-Time Audio Processing
**Challenge**: Audio processing must be deterministic and zero-latency.

**Solution**:
- Used `AVAudioSourceNode` for direct audio processing
- Implemented proper audio format matching
- Used background threads for non-critical operations

#### 3. Parameter Synchronization
**Challenge**: Parameters must be updated in real-time across both platforms.

**Solution**:
- Bidirectional parameter updates
- Efficient change detection and propagation
- Proper state management to avoid excessive updates

## Lessons Learned

### 1. Language Integration Architecture
**Key Decision**: Separation of concerns between Swift and C++.

**Implementation**:
- Layered architecture (View, Model, Manager, Entry Point)
- Clear interface boundaries
- Type-safe communication between layers

**Best Practices**:
- Maintain a single source of truth for critical data
- Use value types for data transfer between threads
- Keep high-performance code in C++

### 2. Audio Architecture Design
**Key Decision**: `AVAudioSourceNode` with zero-latency audio.

**Implementation Details**:
- Audio format: 48kHz, stereo, 32-bit float
- Thread separation: UI thread for interface, audio thread for processing
- Memory management: Proper lifecycle management to prevent glitches

**Technical Challenges**:
- Format matching: Critical for seamless audio integration
- Thread safety: Audio thread must never block UI
- Performance: Profiling revealed bottlenecks in audio path

### 3. UI/UX Design Decisions
**Key Decision**: Maintain visual consistency while providing native macOS experience.

**Implementation**:
- Color language: Uses Magenta's existing color palette
- Typography: Matches Magenta's font styles
- Layout: Consistent spacing and visual hierarchy

**Design Patterns**:
- Component reusability for different hosting scenarios
- Reactive programming for state management
- Proper navigation and accessibility support

### 4. Parameter Handling
**Key Decision**: Bidirectional parameter synchronization.

**Implementation**:
- Swift → C++: Message-based parameter updates
- C++ → Swift: JSON serialization and UI updates
- State management for change detection

**Error Handling**:
- Compile-time type checking
- Runtime validation of parameter ranges
- Graceful degradation on critical errors

### 5. MIDI Integration
**Key Decision**: CoreMIDI for both virtual and physical MIDI support.

**Implementation**:
- Virtual destination: Allows integration with other apps
- Physical input: Direct hardware controller support
- Note tracking: Internal state for pressed notes

**Threading Considerations**:
- Asynchronous MIDI processing on background threads
- Real-time note triggering
- Proper cleanup on disconnection

### 6. Performance Optimization
**Key Decision**: Zero-copy data transfer and caching.

**Implementation**:
- Structs for data transfer (value types)
- Caching of frequently accessed data
- Async operations for non-time-critical tasks

**Profiling Results**:
- Audio thread is most sensitive to delays
- UI thread can tolerate some latency
- Background operations (MIDI, model loading) use async threads

### 7. Testing Strategy
**Key Decision**: Multi-level testing approach.

**Implementation**:
- **Unit Tests**: Test individual components
- **Integration Tests**: Test cross-component interactions
- **Performance Tests**: Test audio continuity and response times

**Test Coverage**:
- View logic and state management
- Manager operations and error handling
- Model structure validation

### 8. Documentation and Onboarding
**Key Decision**: Comprehensive documentation from the start.

**Documentation Created**:
- README.md: High-level overview and setup
- DEVELOPMENT.md: Implementation details and architecture
- Inline comments: Code documentation
- Examples: Usage patterns and best practices

**Onboarding Steps**:
1. Setup: Configure Xcode and dependencies
2. Understanding: Read architectural documents
3. Development: Follow component patterns
4. Testing: Write tests for new features
5. Integration: Test with the C++ engine

## Future Enhancements

### 1. Feature Enhancements
- **Color Themes**: Match system appearance changes (dark/light mode)
- **Audio File Browser**: Native file picker for preloading audio prompts
- **Preset Management**: Save/load complex prompt configurations
- **Performance Monitoring**: Detailed metrics dashboard with alerts
- **Automation Support**: Export/import parameter settings and workflows

### 2. Architecture Improvements
- **Concurrency**: More efficient use of Swift concurrency APIs
- **Testing**: Integration tests for audio path
- **Modularity**: Further separation of concerns

### 3. User Experience
- **Accessibility**: Enhanced support for screen readers and VoiceOver
- **Internationalization**: Support for different languages
- **Customization**: User-configurable themes and layouts

## Building for Production

### Build Process
1. **Dependencies**: Ensure all C++ dependencies are built
2. **Swift Package Manager**: Configure module dependencies
3. **Build Configuration**: Set appropriate target architectures
4. **Code Signing**: Handle macOS code signing requirements

### Deployment Considerations
1. **App Sandbox**: Configure entitlements for proper App Sandbox behavior
2. **Audio Permissions**: System-level audio access requires user permission
3. **MIDI Permissions**: Hardware MIDI access may require special setup
4. **Asset Management**: Proper bundling of model files and resources

## Technical Specifications

### Performance Requirements
- **Audio Latency**: <10ms end-to-end
- **UI Responsiveness**: <16ms frame time
- **Memory Usage**: <500MB peak usage
- **CPU Usage**: <50% on modern Apple Silicon

### Supported Platforms
- **macOS**: 12.0 and later
- **Apple Silicon**: M1 Pro/Max, M2 Pro/Max, M3 Pro/Max, M4 Pro/Max
- **Intel**: Not supported (requires Apple Silicon for real-time)

### Build Configuration
- **Swift**: 5.5+
- **Xcode**: 14.0+
- **Target**: macOS 12.0+
- **Deployment**: arm64 architecture (Apple Silicon)

## Testing Guidelines

### Unit Tests
```swift
// Test SwiftUI view state changes
// Test manager operations
// Test model structure validation
```

### Integration Tests
```swift
// Test audio continuity across parameter changes
// Test MIDI to audio conversion
// Test parameter synchronization
```

### Performance Tests
```swift
// Test audio dropouts
// Test UI responsiveness under load
// Test memory usage patterns
```

## Contributing

This Swift player is designed to be extended and integrated with the broader Magenta ecosystem:

1. **Add New Features**: Extend the PlayerManager for new functionality
2. **Improve Integration**: Enhance the C++ interface for better control
3. **Documentation**: Update documentation and examples
4. **Testing**: Add comprehensive test coverage

### Coding Standards
- **Swift**: Follow Swift Evolution guidelines
- **Architecture**: Maintain the layered architecture pattern
- **Performance**: Profile and optimize critical paths
- **Documentation**: Write clear, comprehensive documentation

## Conclusion

The Magenta Swift player provides a powerful native macOS interface for real-time music generation. Its key strengths are:

1. **Seamless Integration**: Works directly with the Magenta C++ inference engine
2. **Native Performance**: Leverages Apple Silicon for optimal performance
3. **Developer Experience**: Comprehensive documentation and testing
4. **Production Ready**: Designed with proper architecture and error handling

This project serves as a foundation for future Swift-based applications for Magenta and provides a blueprint for integrating native UI with high-performance C++ audio processing.